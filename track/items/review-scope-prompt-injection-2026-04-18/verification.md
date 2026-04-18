# Verification: review-scope-prompt-injection-2026-04-18

## Scope Verified

- [ ] In-scope items match ADR 2026-04-18-1354-review-scope-prompt-injection §D1-§D5 (Phase 1-3 のみ)
- [ ] Out-of-scope items correctly deferred (Phase 4 他 scope 展開 / .harness/config/ 集約 / CI lint / research-notes を独立 scope に分離する案 (knowledge/research/** は plan-artifacts に統合済み) / Open Q2 empty diff 方針転換)
- [ ] 既存 review-scope.json (briefing_file なし) が後方互換で動作する
- [ ] Other scope が briefing 対象外であることが API レベルで保証される (briefing_file_for_scope(ScopeName::Other) → None)

## Task Verification

### T001: ScopeEntry 追加 + ReviewScopeConfig 内部変更 (domain)

- [x] `libs/domain/src/review_v2/scope_config.rs` に `ScopeEntry` struct (crate-private) が追加されている
- [x] `ReviewScopeConfig.scopes` の型が `HashMap<MainScopeName, ScopeEntry>` に変更されている
- [x] `ReviewScopeConfig::new` シグネチャが `entries: Vec<(String, Vec<String>, Option<String>)>` に拡張されている
- [x] `ReviewScopeConfig::new` の entries loop で各 pattern に `expand_track_id` が適用されている (group pattern の `<track-id>` placeholder が current track に展開される — ADR D3 の既存ルール準拠、pre-T001 の limitation を解消)
- [x] `briefing_file_for_scope(&self, scope: &ScopeName) -> Option<&str>` アクセサが追加され、`ScopeName::Other` で必ず `None` を返す
- [x] `classify` / `contains_scope` / `all_scope_names` の既存 unit tests が全 pass
- [x] group pattern `track/items/<track-id>/**` が current track の spec.md にマッチする unit test を追加 (placeholder 展開の regression guard — `test_scope_config_group_pattern_expands_track_id_placeholder`)
- [x] `libs/usecase/src/review_v2/tests.rs:155` の `ReviewScopeConfig::new` 呼び出しを 3-tuple 形に更新済み (cross-layer call site — 見落とすと CI が break する)
- [x] `libs/infrastructure/src/review_v2/scope_config_loader.rs:119` の `ReviewScopeConfig::new` 呼び出しを 3-tuple 形に更新済み (loader 呼び出し側は briefing_file に暫定 `None` を渡す。T002 で `GroupEntry.briefing_file` の serde field を追加して実値を流す)
- [x] `ReviewScopeConfig` への `#[derive(Clone)]` 追加: T001 時点で `compose_v2.rs` の呼び出し側が `scope_config` を clone しないため不要と判断し追加しなかった。T003 で `append_scope_briefing_reference` を実装する際に clone が必要になれば追加する (Open Q-IMPL-02 の T001 段階での解答)
- [x] `cargo make ci` 全 green (T001 の変更のみ、2147 tests passed — T002-T008 未実装時点の中間 CI。全タスク完了後の最終 CI は T008 で確認する)

### T002: GroupEntry に briefing_file 追加 (infrastructure loader)

- [x] `GroupEntry` に `briefing_file: Option<String>` フィールドが `#[serde(default)]` 付きで追加されている
- [x] 既存 review-scope.json (briefing_file なし) が引き続き load できる (後方互換テスト — `test_load_without_briefing_file_is_backward_compatible`)
- [x] briefing_file 付き JSON が正しく parse され `briefing_file_for_scope` が `Some` を返す (`test_load_with_briefing_file_populates_accessor`)
- [x] typo フィールド (`briefng_file` 等) が Parse エラーで reject される (deny_unknown_fields regression guard — `test_typo_in_briefing_file_field_is_rejected`)
- [x] `cargo make ci` 全 green (2150 tests passed, T002 で +3 件追加)

### T003: briefing composer に scope briefing 参照行 append (cli)

- [x] `apps/cli/src/commands/review/codex_local.rs` に `append_scope_briefing_reference` pure 関数が追加されている
- [x] 出力 format が ADR D4 Canonical Block (`## Scope-specific severity policy` 見出し + Read 指示 + path) に完全一致
- [x] `briefing_file` が Some / None / Other / unknown-main の 4 ケースで append / noop が期待通り動作する unit test + prompt injection guard (path に改行・バッククォート・空文字) の 3 件 = 計 7 件の unit test が全 pass
- [x] `run_execute_codex_local` の実行フロー順序問題が解決されている (option B 採用、下記 §Implementation Notes 参照)
- [x] `cargo make ci` 全 green (2157 tests passed, T003 で +7 件追加)

#### Implementation Notes

- [x] 採用した順序改修アプローチ: **option B** — `apps/cli/src/commands/review/compose_v2.rs` に `load_scope_config_only(track_id, items_dir) -> Result<ReviewScopeConfig, String>` を新設 (scope_json_path 作成 + `load_v2_scope_config` 呼び出し + items_dir 正規化を独立実装。`build_v2_shared` は引き続き同等のロジックを内部で実行するため、scope_config の読み込みは 2 回発生するが、pure な glob コンパイルのみで副作用がないため許容)。`run_execute_codex_local` を以下の 6 ステップに再編: validate → track_id + map_group → load_scope_config_only → build_base_prompt + append_scope_briefing_reference → CodexReviewer::new → build_review_v2_with_reviewer。`ReviewScopeConfig: Clone` / `CodexReviewer::with_scope_briefing` builder (option A/C) を追加せずに済み、domain / infrastructure / 既存 composition 構造に破壊的変更なし

### T004: review-fix-lead agent prompt 更新

- [x] `.claude/agents/review-fix-lead.md` に `## Scope-specific severity policy` 段落が `## Workflow` 直前に追加されている
- [x] 段落に「主 briefing に該当節があれば必ず Read ツールで読み込み、severity filter 根拠とし、毎回 fresh に読む」旨が明記されている
- [x] agent prompt に Read 指示が組み込まれている (runtime observation は T008 §Dogfooding Result に記録予定)

### T005: /track:review command doc 更新

- [x] `.claude/commands/track/review.md` Step 2b に scope briefing 自動注入の説明が追加されている
- [x] briefing author (review-fix-lead) と composer の責任分離が明記されている (scope-specific severity policy 節は手で書かない旨)
- [x] scope リストに plan-artifacts が追記されている (output example + 現行 named groups 列挙)
- [x] `cargo make verify-arch-docs` / `cargo make verify-doc-links` が通る (T004-T007 一括 CI green)

### T006: review-scope.json に plan-artifacts scope 追加

- [x] `track/review-scope.json` の plan-artifacts エントリが最終形に更新されている (bootstrap `track/items/**` → `track/items/<track-id>/**` 切り替え)
- [x] `patterns`: `["track/items/<track-id>/**", "knowledge/adr/**", "knowledge/research/**"]`
- [x] `briefing_file`: `"track/review-prompts/plan-artifacts.md"`
- [x] T001 の loader fix (expand_track_id on groups) が前提として merge されている (commit `c4afff6...`)
- [x] 既存 scope 定義 (domain / usecase / infrastructure / cli / harness-policy) は変更されていない
- [x] Integration test (accepted deviation): T006 仕様が指定した infrastructure 統合テスト (live `track/review-scope.json` ロード + 3 パス分類 assert) は別途追加せず、T008 ドッグフード (live data での end-to-end 検証) で代替することを受け入れた。単体レベルでは `test_scope_config_group_pattern_expands_track_id_placeholder` (T001) が `<track-id>` 展開を、`test_load_with_briefing_file_populates_accessor` (T002) が `briefing_file_for_scope` の Some 返却を確認済み

### T007: track/review-prompts/plan-artifacts.md 新規作成

- [x] `track/review-prompts/` ディレクトリが新規作成されている
- [x] `track/review-prompts/plan-artifacts.md` が存在する
- [x] What to report (5 カテゴリ: factual error / contradiction / broken reference / infeasibility / timestamp inconsistency) と What NOT to report (4 カテゴリ: wording nits / EN-JP mix / alternative design / formatting) の 2 セクションが含まれている (Round budget / round 数 cap は含めず)
- [x] markdown が self-contained (reviewer が他 doc 参照せず適用可能)
- [x] `briefing_file` の CI lint (broken link 検知) は Open Question Q3 として defer 済み。ファイルは存在し reviewer が Read ツールで読める状態にある (runtime observation は T008 §Dogfooding Result に記録予定)

### T008: CI 通過 + ドッグフード

- [ ] `cargo make ci` が全 green (fmt-check + clippy + nextest + test-doc + deny + python-lint + scripts-selftest + check-layers + verify-arch-docs + verify-doc-links)
- [ ] 本 track の `/track:review` で `track/items/review-scope-prompt-injection-2026-04-18/**` と改訂 ADR (knowledge/adr/2026-04-18-1354-*.md) と planner 研究ノート (knowledge/research/2026-04-18-0514-*.md) が plan-artifacts scope に自動分類される
- [ ] briefing composer が `## Scope-specific severity policy` 節を自動追加する
- [ ] reviewer が Read ツールで `track/review-prompts/plan-artifacts.md` を読み込む
- [ ] severity policy 適用後 zero_findings を返す (wording nit 起因の finding が 0 件)
- [ ] 結果 (scope 別ファイル数 / 各 scope の round 数 / severity policy 適用確認) を本ファイル §Dogfooding Result に記録

#### Dogfooding Result

実装完了後に記入:

- [ ] plan-artifacts scope に分類されたファイル数: `TBD`
- [ ] harness-policy scope に分類されたファイル数: `TBD`
- [ ] domain / infrastructure / cli 各 scope に分類されたファイル数: `TBD`
- [ ] 各 scope の final model review round 数: `TBD`
- [ ] plan-artifacts.md が reviewer に読まれたか (Read tool invocation 確認): `TBD`
- [ ] wording nit 起因の finding が 0 件であること: `TBD`

## Result

_TBD — will be recorded after T008 dogfooding succeeds_

## Open Issues

- **Open Q-IMPL-01**: ~~`CodexReviewer::base_prompt` は private。注入方法の最終選択 (A/B/C) は T003 実装時に決定し、本ファイル Implementation Notes に記録する~~ **T003 完了により解決済み**: option B を採用 (`load_scope_config_only` を pre-load して `base_prompt` 生成前に注入)。Implementation Notes に詳細記録済み
- **Open Q-IMPL-02**: ~~`ReviewScopeConfig` の `Clone` derive 追加の是非は T001 / T003 実装時に決定する~~ **T003 完了により解決済み**: option B 採用により `Clone` は不要と確定。`load_scope_config_only` で `build_review_v2_with_reviewer` が内部で再読み込みするため clone なし。`ReviewScopeConfig: Clone` / `CodexReviewer::with_scope_briefing` builder は追加しなかった
- **副作用**: 既存 track の `review.json` で `other` scope hash が plan-artifacts 追加後 StaleHash となる (正常挙動、新 scope 境界の再計算)
- **commit 分類**: 本 track の commit は `track/review-scope.json` 変更により `harness-policy` scope と、`track/items/` / `knowledge/adr/` / `knowledge/research/` 変更により `plan-artifacts` scope で review される (bootstrap 適用済み)
- **Bootstrap 状態**: T001 + T006 完了済み。plan-artifacts patterns は最終形 `["track/items/<track-id>/**", "knowledge/adr/**", "knowledge/research/**"]` に切り替え済み。`briefing_file: "track/review-prompts/plan-artifacts.md"` も設定済み

## Verified At

_TBD — verification will be recorded after T008 dogfooding succeeds._
