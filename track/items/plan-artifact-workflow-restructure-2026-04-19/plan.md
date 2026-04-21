<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# 計画成果物ワークフローの再構築 (Scope D: T1 + T2 + T3)

## Summary

計画成果物ワークフローの構造的刷新 (ADR: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md) の Scope D (T1 + T2 + T3) を実装する
新 domain 型 (値オブジェクト newtype 6 種 + 4 独立 ref 構造体、libs/domain/src/plan_ref/ 配置) を基盤に、spec.json / 型カタログ / metadata / impl-plan / task-coverage のスキーマを再構成する
verify-*-local を file 存在ベースに改訂、空カタログを受け入れ、task-completion gate (check_tasks_resolved_from_git_ref) を impl-plan.json 読み替え、plan.md / spec.md renderer を集約形式に変更する
sotp verify plan-artifact-refs CLI を新設し、既存 spec_coverage::verify を統合、canonical block 疑惑検出を追加し cargo make ci に組み込む
独立 phase コマンド (/track:init / /track:spec / /track:impl-plan)、/track:design 責務刷新、/track:plan orchestrator + adr-editor capability、hook 強制は別 track (T4-T8)

## Tasks (4/12 resolved)

### S1 — S1 — 明文化 (T1 相当)

> convention 2 本 (workflow-ceremony-minimization.md / pre-track-adr-authoring.md) を新設する
> CLAUDE.md / DEVELOPER_AI_WORKFLOW.md / track/workflow.md に track 前段階 + 3 フェーズを明記する
> SKILL / command / agent (planner, designer) から approved 廃止 + ADR 事前確認 + D1.6 research 配置 convention を反映する
> adr-editor capability の新設は本 track 範囲外 (T7.5 別 track)

- [x] **T001**: convention 新設 (knowledge/conventions/workflow-ceremony-minimization.md, knowledge/conventions/pre-track-adr-authoring.md) + CLAUDE.md / DEVELOPER_AI_WORKFLOW.md / track/workflow.md に track 前段階 + 3 フェーズ明記 + .claude/skills/track-plan/SKILL.md / .claude/commands/track/plan.md / .claude/commands/track/design.md / .claude/agents/planner.md / .claude/agents/designer.md から approved 廃止 + ADR 事前確認追加 + D1.6 research note 配置 convention を適用 (`999e954a86e8fb5d95f6b090c9b8c52b52b581f8`)

### S2 — S2 — 新 domain 型の導入 (T2 相当)

> libs/domain/src/plan_ref/ 新モジュール (ref 種別ごとに 1 ファイル: adr_ref.rs / convention_ref.rs / spec_ref.rs / informal_ground_ref.rs + mod.rs) を導入
> 値オブジェクト newtype 6 種: SpecElementId / AdrAnchor / ConventionAnchor / ContentHash / InformalGroundKind / InformalGroundSummary
> 4 独立 ref 構造体: AdrRef { file, anchor: AdrAnchor } / ConventionRef { file, anchor: ConventionAnchor } / SpecRef { file, anchor: SpecElementId, hash: ContentHash } / InformalGroundRef { kind: InformalGroundKind, summary: InformalGroundSummary }
> 各 newtype のコンストラクタで validation を閉じ込め、使用サイトに Option<String> を露出させない
> role 文字列タグ・discriminated union・共通 trait 抽象化は導入しない (独立 4 構造体の原則)
> InformalGroundRef は file 対象を持たず、未永続化根拠 (議論 / feedback / memory / user directive) を構造化して citing 可能にし、signal 評価で 🟡 を発火する

- [x] **T002**: libs/domain/src/plan_ref/ 新モジュールを導入 (ref 種別ごとに 1 ファイル構成: mod.rs + adr_ref.rs + convention_ref.rs + spec_ref.rs + informal_ground_ref.rs)。値オブジェクト newtype 6 種 (SpecElementId / AdrAnchor / ConventionAnchor / ContentHash / InformalGroundKind / InformalGroundSummary) と 4 独立 ref 構造体 (AdrRef / ConventionRef / SpecRef / InformalGroundRef) を追加する。SpecElementId は非空文字列 + ID 命名規則 (例: IN-\d+, AC-\d+ 等) で validation、AdrAnchor / ConventionAnchor は非空文字列のみ (loose、Q15 で厳密化)、ContentHash は SHA-256 形式 (32 バイト) で validation、InformalGroundKind は 4 variant enum (Discussion / Feedback / Memory / UserDirective)、InformalGroundSummary は非空一行要約で validation。各コンストラクタに validation を閉じ込める。既存の libs/domain/src/ids.rs (entity identity 専用) とは分離する (NonEmptyString の流用はしない)。unit tests 付与 (`015dd1374a18eac9967c0975c4148448e2671a8e`)

### S3 — S3 — スキーマ再構成 (T2 相当)

> spec.json: status / approved_at / トップレベル content_hash / 各要素 task_refs を削除、各要素 id 必須化、sources → adr_refs + convention_refs + informal_grounds の 3 分割、related_conventions を ConventionRef に
> impl-plan.json: schema_version + tasks + plan の新 document として分離
> task-coverage.json: spec 4 セクションごとの task_refs を保持する新 document
> 型カタログ: spec_refs: Vec<SpecRef> + informal_grounds: Vec<InformalGroundRef> field を追加
> metadata.json: identity-only に縮小 (tasks / plan を削除)
> schema_version 移行戦略は実装者判断 (既存 track 互換対応を含む)

- [x] **T003**: libs/domain/src/spec.rs を刷新 (status / approved_at / トップレベル content_hash / 各要素 task_refs を削除、各要素に id: SpecElementId 必須化、現行の sources を adr_refs: Vec<AdrRef> + convention_refs: Vec<ConventionRef> + informal_grounds: Vec<InformalGroundRef> の 3 分割、top-level related_conventions を Vec<ConventionRef> に)。libs/infrastructure/src/spec/codec.rs の serde 更新。spec-signals ツールの入力抽出経路も新 field に合わせて更新 (adr_refs と informal_grounds の signal 合成も含む)。domain + infra の既存 tests を新 schema で書き換え (`6774e9f587e4d48df8a38e611027431c7c828bb7`)
- [x] **T004**: libs/domain/ に ImplPlanDocument (schema_version + tasks + plan) と TaskCoverageDocument (4 セクション: in_scope / out_of_scope / constraints / acceptance_criteria の要素ごとの task_refs) を新設。既存 TrackTask / PlanView / PlanSection を流用。libs/infrastructure/ に両 document の codec 新設。unit tests (`3d915200e088161b8923d45967b9279d647440a8`)
- [~] **T005**: 型カタログドキュメントに spec_refs: Vec<SpecRef> + informal_grounds: Vec<InformalGroundRef> field を追加。libs/domain/src/track.rs の TrackMetadata から tasks / plan を削除し identity-only 化 (schema migration 戦略は実装者判断、既存 v3 との並立対応を含む)。両方の codec 更新 + tests

### S4 — S4 — verify / gate 移行 (T2 相当)

> verify-track-metadata-local を identity 専用検証に
> verify-latest-track-local を impl-plan.json 存在条件で task 項目チェック切替
> sotp track type-signals / baseline-capture から空カタログ拒否を撤廃
> check_tasks_resolved_from_git_ref を metadata.json 読みから impl-plan.json 読みに切り替え (TrackBlobReader port 改修 + K1-K7 tests)

- [ ] **T006**: libs/infrastructure/src/verify/ の verify-track-metadata-local / verify-latest-track-local を file 存在ベースに改訂 (metadata = identity のみ検証、latest-track = impl-plan.json 存在条件で task 項目チェック)。sotp track type-signals / baseline-capture / 関連 verify から「空カタログ拒否」ロジックを撤廃し、エントリ 0 件の空カタログを有効状態として受け入れ。tests 更新
- [~] **T007**: libs/usecase/src/task_completion.rs の check_tasks_resolved_from_git_ref を metadata.json から impl-plan.json 読み替え。TrackBlobReader port の read_track_metadata を read_impl_plan に置換 or 追加、apps/cli/src/commands/pr.rs 呼び出し側も改修。K1-K7 MockReader tests を新 port で書き換え

### S5 — S5 — renderer + 旧 gate 廃止 (T2 相当)

> plan.md renderer を metadata.json + impl-plan.json の集約に
> spec.md renderer を spec.json + task-coverage.json の集約に
> cargo make spec-approve 廃止、approved 概念の消滅に追従する spec-signals / schema 参照コードの更新

- [~] **T008**: libs/infrastructure/src/track/render.rs の plan.md / spec.md renderer を集約形式に変更 (plan.md = metadata.json + impl-plan.json、spec.md = spec.json + task-coverage.json)。tests 更新
- [ ] **T009**: Makefile.toml から cargo make spec-approve タスクを削除し、関連する apps/cli/src/commands/ の spec approve コマンドを廃止。spec-signals ツールおよび spec schema 参照コードから status / approved_at / content_hash 関連の依存を除去。tests 整理

### S6 — S6 — plan-artifact-refs CLI (T3 相当)

> sotp verify plan-artifact-refs subcommand 新設 (ref field 走査 + schema / file 存在 + SpecRef.anchor 解決 + SpecRef.hash 照合 + AdrAnchor / ConventionAnchor loose validation)
> task-coverage.json の coverage 強制 + referential integrity 検査 (現行 spec_coverage::verify 踏襲)
> canonical block 疑惑検出 (ADR 例示マーカー除外 / spec 内 10 行超コードブロック警告)
> cargo make ci への組み込み、既存 spec_coverage::verify 呼び出し経路の新 CLI への移行

- [ ] **T010**: apps/cli/src/commands/verify/plan_artifact_refs.rs を新設し sotp verify plan-artifact-refs subcommand として公開。各 ref field (adr_refs / convention_refs / spec_refs / related_conventions / informal_grounds) の走査、schema validation、file 存在チェック (file ベース ref のみ)、SpecRef.anchor 解決 (spec 要素 id lookup)、SpecRef.hash 照合 (canonical JSON subtree SHA-256)、AdrAnchor / ConventionAnchor の newtype loose validation、InformalGroundRef の newtype validation (kind variant / summary 非空、file resolution なし) を実装。unit + integration tests
- [ ] **T011**: T010 の CLI に task-coverage.json の coverage 強制 + referential integrity 検査 (現行 spec_coverage::verify 踏襲) を追加し、canonical block 疑惑検出 (ADR 例示マーカー除外 / spec フィールド内 10 行超コードブロック警告) を実装。cargo make ci / Makefile.toml に組み込み、CI で fail-closed 動作を検証
- [ ] **T012**: 既存 libs/infrastructure/src/verify/spec_coverage.rs の spec_coverage::verify を新 CLI (plan-artifact-refs) に統合し、旧呼び出し経路 (Makefile.toml / scripts/ / 関連 verify-*) を新 CLI 経由に切り替え、不要コードを削除。tests 整理
