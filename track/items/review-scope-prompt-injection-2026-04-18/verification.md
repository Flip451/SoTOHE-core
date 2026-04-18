# Verification: review-scope-prompt-injection-2026-04-18

## Scope Verified

- [ ] In-scope items match ADR 2026-04-18-1354-review-scope-prompt-injection §D1-§D5 (Phase 1-3 のみ)
- [ ] Out-of-scope items correctly deferred (Phase 4 他 scope 展開 / .harness/config/ 集約 / CI lint / research-notes を独立 scope に分離する案 (knowledge/research/** は plan-artifacts に統合済み) / Open Q2 empty diff 方針転換)
- [ ] 既存 review-scope.json (briefing_file なし) が後方互換で動作する
- [ ] Other scope が briefing 対象外であることが API レベルで保証される (briefing_file_for_scope(ScopeName::Other) → None)

## Task Verification

### T001: ScopeEntry 追加 + ReviewScopeConfig 内部変更 (domain)

- [ ] `libs/domain/src/review_v2/scope_config.rs` に `ScopeEntry` struct (crate-private) が追加されている
- [ ] `ReviewScopeConfig.scopes` の型が `HashMap<MainScopeName, ScopeEntry>` に変更されている
- [ ] `ReviewScopeConfig::new` シグネチャが `entries: Vec<(String, Vec<String>, Option<String>)>` に拡張されている
- [ ] `ReviewScopeConfig::new` の entries loop で各 pattern に `expand_track_id` が適用されている (group pattern の `<track-id>` placeholder が current track に展開される — ADR D3 の既存ルール準拠、pre-T001 の limitation を解消)
- [ ] `briefing_file_for_scope(&self, scope: &ScopeName) -> Option<&str>` アクセサが追加され、`ScopeName::Other` で必ず `None` を返す
- [ ] `classify` / `contains_scope` / `all_scope_names` の既存 unit tests が全 pass
- [ ] group pattern `track/items/<track-id>/**` が current track の spec.md にマッチする unit test を追加 (placeholder 展開の regression guard)
- [ ] `libs/usecase/src/review_v2/tests.rs:155` の `ReviewScopeConfig::new` 呼び出しを 3-tuple 形に更新済み (cross-layer call site — 見落とすと CI が break する)

### T002: GroupEntry に briefing_file 追加 (infrastructure loader)

- [ ] `GroupEntry` に `briefing_file: Option<String>` フィールドが `#[serde(default)]` 付きで追加されている
- [ ] 既存 review-scope.json (briefing_file なし) が引き続き load できる (後方互換テスト)
- [ ] briefing_file 付き JSON が正しく parse され `briefing_file_for_scope` が `Some` を返す
- [ ] typo フィールド (`briefng_file` 等) が Parse エラーで reject される (deny_unknown_fields regression guard)

### T003: briefing composer に scope briefing 参照行 append (cli)

- [ ] `apps/cli/src/commands/review/codex_local.rs` に `append_scope_briefing_reference` pure 関数が追加されている
- [ ] 出力 format が ADR D4 Canonical Block (`## Scope-specific severity policy` 見出し + Read 指示 + path) に完全一致
- [ ] `briefing_file` が Some / None / Other の 3 ケースで append / noop が期待通り動作する unit test が全 pass
- [ ] `run_execute_codex_local` の実行フロー順序問題が解決されている (採用した選択肢 A/B/C を本ファイル §Implementation Notes に記録)

#### Implementation Notes

- [ ] 採用した順序改修アプローチ: (A) `CodexReviewer::with_scope_briefing` builder / (B) scope_config pre-load / (C) composition 保持 → 実装時に記入

### T004: review-fix-lead agent prompt 更新

- [ ] `.claude/agents/review-fix-lead.md` に `## Scope-specific severity policy` 段落が `## Workflow` 直前に追加されている
- [ ] 段落に「主 briefing に該当節があれば必ず Read ツールで読み込み、severity filter 根拠とし、毎回 fresh に読む」旨が明記されている
- [ ] 本 track の review でドッグフード時に agent が実際に plan-artifacts.md を Read する挙動を確認

### T005: /track:review command doc 更新

- [ ] `.claude/commands/track/review.md` Step 2b に scope briefing 自動注入の説明が追加されている
- [ ] briefing author (review-fix-lead) と composer の責任分離が明記されている
- [ ] scope リストに plan-artifacts が追記されている
- [ ] `cargo make verify-arch-docs` / `cargo make verify-doc-links` が通る

### T006: review-scope.json に plan-artifacts scope 追加

- [ ] `track/review-scope.json` の plan-artifacts エントリが最終形に更新されている (bootstrap `track/items/**` → `track/items/<track-id>/**` 切り替え)
- [ ] `patterns`: `["track/items/<track-id>/**", "knowledge/adr/**", "knowledge/research/**"]`
- [ ] `briefing_file`: `"track/review-prompts/plan-artifacts.md"`
- [ ] T001 の loader fix (expand_track_id on groups) が前提として merge されている
- [ ] 既存 scope 定義 (domain / usecase / infrastructure / cli / harness-policy) は変更されていない
- [ ] Integration test: `track/items/<current>/spec.md` と `knowledge/adr/xxxx.md` と `knowledge/research/xxxx.md` が plan-artifacts に分類され、`briefing_file_for_scope` が Some を返す

### T007: track/review-prompts/plan-artifacts.md 新規作成

- [ ] `track/review-prompts/` ディレクトリが新規作成されている
- [ ] `track/review-prompts/plan-artifacts.md` が存在する
- [ ] What to report / What NOT to report の 2 セクションが含まれている (Round budget / round 数 cap は **含めない** — orchestrator pacing に属するため severity policy から除外)
- [ ] markdown が self-contained (reviewer が他 doc 参照せず適用可能)
- [ ] `briefing_file` の CI lint (broken link 検知) は Open Question Q3 として defer 済みのため verify-doc-links による検証は対象外。T008 のドッグフードサイクルで reviewer が Read ツールで `track/review-prompts/plan-artifacts.md` を読み込めることを実証することで代替

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

_TBD — all tasks complete_

## Open Issues

- **Open Q-IMPL-01**: `CodexReviewer::base_prompt` は private。注入方法の最終選択 (A/B/C) は T003 実装時に決定し、本ファイル Implementation Notes に記録する
- **Open Q-IMPL-02**: `ReviewScopeConfig` の `Clone` derive 追加の是非は T001 / T003 実装時に決定する (`GlobMatcher` が Clone 可能なことは globset crate で確認済み)
- **副作用**: 既存 track の `review.json` で `other` scope hash が plan-artifacts 追加後 StaleHash となる (正常挙動、新 scope 境界の再計算)
- **commit 分類**: 本 track の commit は `track/review-scope.json` 変更により `harness-policy` scope と、`track/items/` / `knowledge/adr/` / `knowledge/research/` 変更により `plan-artifacts` scope で review される (bootstrap 適用済み)
- **Bootstrap 状態**: T001 未実装 (loader が group pattern で `<track-id>` を展開しない) の制約下で現在の plan-artifacts patterns は `["track/items/**", "knowledge/adr/**", "knowledge/research/**"]` (literal `**`) を使用している。T006 で T001 完了後に `track/items/<track-id>/**` へ切り替える前提

## Verified At

_TBD — verification will be recorded after T008 dogfooding succeeds._
