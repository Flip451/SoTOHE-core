<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# review-scope.json に scope 別 briefing 注入機構を追加する — plan-artifacts scope の新設

ADR 2026-04-18-1354 (review-scope-prompt-injection) の Phase 1-3 を実装する
Phase 1 (schema): ScopeEntry 追加 + ReviewScopeConfig 内部型変更 + GroupEntry に briefing_file field 追加 (後方互換)
Phase 2 (injection): briefing composer に scope briefing 参照行 append 処理追加 + review-fix-lead agent prompt 更新 + /track:review doc 更新
Phase 3 (plan-artifacts): review-scope.json に plan-artifacts group 追加 + track/review-prompts/plan-artifacts.md 新規作成
ADR D4 採用方式: briefing は参照行 1 本を push_str するのみ、reviewer の Read tool がファイル内容を取りに行く (本文連結 / fs::read / trusted_root check は一切行わない)
ADR Phase 4 (他 scope への展開) は scope 外。本 track 完了後、別 track で harness-policy / domain などへ順次展開予定

## S001 — ScopeEntry 追加 + ReviewScopeConfig 内部変更 (domain)

libs/domain/src/review_v2/scope_config.rs に ScopeEntry struct (crate-private) を追加する
ReviewScopeConfig.scopes の型を HashMap<MainScopeName, Vec<GlobMatcher>> から HashMap<MainScopeName, ScopeEntry> に変更する
ReviewScopeConfig::new シグネチャを entries: Vec<(String, Vec<String>, Option<String>)> に拡張する (第 3 要素が briefing_file パス)
classify / contains_scope / all_scope_names / is_other_track などの内部参照を ScopeEntry.matchers 経由に書き換える
briefing_file_for_scope(&self, scope: &ScopeName) -> Option<&str> アクセサを追加する (ScopeName::Other は常に None)
unit tests: ScopeEntry の Some/None 両ケース、briefing_file_for_scope の Some/None/Other ケース

- [ ] Add ScopeEntry struct (crate-private) to libs/domain/src/review_v2/scope_config.rs and change ReviewScopeConfig.scopes type from HashMap<MainScopeName, Vec<GlobMatcher>> to HashMap<MainScopeName, ScopeEntry>. Extend ReviewScopeConfig::new signature to accept entries: Vec<(String, Vec<String>, Option<String>)> where the third element is the optional briefing_file path. In the entries loop, expand <track-id> placeholder in each pattern before compiling the glob (per ADR D3: 'patterns は <track-id> placeholder を既存ルール通り展開する (既存 review_operational と同じ挙動)') — currently the entries loop compiles patterns verbatim without expansion, so this expansion must be added in T001. Rewrite internal references in classify / contains_scope / all_scope_names / is_other_track to go through ScopeEntry.matchers. Add briefing_file_for_scope(&self, scope: &ScopeName) -> Option<&str> accessor on ReviewScopeConfig: ScopeName::Other always returns None per ADR D5; ScopeName::Main(name) returns the scope's briefing_file path if configured. Add #[derive(Clone)] to ReviewScopeConfig if not already present (GlobMatcher is Clone). Update all ReviewScopeConfig::new call sites to the 3-tuple form: libs/domain/src/review_v2/scope_config.rs (implementation + #[cfg(test)] tests), libs/domain/src/review_v2/tests.rs (13+ invocations), and libs/usecase/src/review_v2/tests.rs:155 (cross-layer test that also constructs ReviewScopeConfig directly; must be updated in this task to avoid CI failure when the domain signature changes). Unit tests: ScopeEntry with briefing_file Some/None, classify() unchanged behavior with ScopeEntry-backed scopes, briefing_file_for_scope returns the correct path for Main scopes with briefing, None for Main scopes without briefing, and None for Other.

## S002 — GroupEntry に briefing_file 追加 (infrastructure loader)

libs/infrastructure/src/review_v2/scope_config_loader.rs の GroupEntry に briefing_file: Option<String> を #[serde(default)] 付きで追加する
load_v2_scope_config の entries 組み立て部分を (name, entry.patterns, entry.briefing_file) の 3-tuple に変更する
unit tests: briefing_file 付き JSON を正しくパースし briefing_file_for_scope が Some を返すこと、briefing_file なし既存 JSON が引き続き動くこと (後方互換)、typo フィールド (briefng_file 等) が deny_unknown_fields で reject されること

- [ ] Add briefing_file: Option<String> field to GroupEntry in libs/infrastructure/src/review_v2/scope_config_loader.rs with #[serde(default)] so absence is equivalent to None. Update the entries assembly in load_v2_scope_config to produce (name, entry.patterns, entry.briefing_file) 3-tuples, matching the new ReviewScopeConfig::new signature from T001. Preserve deny_unknown_fields at the ReviewScopeJsonV2 top level and at GroupEntry. Unit tests: (1) loading a review-scope.json with a briefing_file entry produces a ReviewScopeConfig where briefing_file_for_scope returns Some(path) for that scope; (2) loading the existing review-scope.json without briefing_file continues to work (backward compatibility); (3) a typo field like briefng_file in a GroupEntry is rejected with a Parse error (deny_unknown_fields regression guard).

## S003 — briefing composer に scope briefing 参照行 append (cli)

apps/cli/src/commands/review/codex_local.rs に append_scope_briefing_reference(prompt: &mut String, scope: &ScopeName, scope_config: &ReviewScopeConfig) pure 関数を追加する
ReviewScopeConfig の briefing_file_for_scope が Some(path) を返す場合、prompt に `## Scope-specific severity policy` 節を append する。Read ツールで file を読むよう reviewer に指示する文面を含める (ADR D4 Canonical Block に準拠)
run_execute_codex_local の実行フロー順序問題を解決する (現状: base_prompt 生成 → composition 構築の順、scope_config が base_prompt 生成時にアクセス不可)。解決方針は実装時に選択: (A) CodexReviewer に with_scope_briefing(path) builder 追加、(B) scope_config を pre-load して base_prompt 生成前に注入、(C) ReviewV2CompositionWithCodex に scope_config 保持
unit tests: append_scope_briefing_reference が briefing_file が Some/None で期待通りの append/noop をすること、output format が Canonical Block (`## Scope-specific severity policy` 見出し + Read 指示) に準拠すること、ScopeName::Other では必ず noop になること

- [ ] Add append_scope_briefing_reference(prompt: &mut String, scope: &ScopeName, scope_config: &ReviewScopeConfig) pure function to apps/cli/src/commands/review/codex_local.rs. When scope_config.briefing_file_for_scope(scope) returns Some(path), append a `## Scope-specific severity policy` section to prompt that instructs the reviewer to Read the file at path before selecting findings. The appended format must match the example block in ADR D4 (knowledge/adr/2026-04-18-1354-review-scope-prompt-injection.md §D4): heading `## Scope-specific severity policy` + Japanese instruction line + bulleted path reference. When None, the function is a no-op. Solve the ordering problem in run_execute_codex_local where base_prompt is currently built in Step 2 before ReviewScopeConfig is loaded in Step 3 (inside build_review_v2_with_reviewer). Implementation options (pick during implementation): (A) add CodexReviewer::with_scope_briefing(briefing_path) builder and chain after composition; (B) pre-load scope_config via a new helper so base_prompt can be augmented before CodexReviewer::new; (C) have ReviewV2CompositionWithCodex retain scope_config and mutate reviewer post-construction via a builder. Record the chosen approach in verification.md. Unit tests: append_scope_briefing_reference with briefing_file Some appends the expected section; with None prompt is unchanged; with ScopeName::Other prompt is unchanged regardless of config; output format exactly matches the ADR D4 example block.

## S004 — review-fix-lead agent prompt 更新

.claude/agents/review-fix-lead.md に `## Scope-specific severity policy` 段落を `## Workflow` 見出しの直前に追加する
内容: 主 briefing に `## Scope-specific severity policy` 節がある場合、必ず review 開始前に Read tool でそのファイルを読むこと、severity filter 適用の根拠となること、セッション間で更新され得るため毎回 fresh に読むことを明記する
テスト: 自動テスト不要 (agent prompt は実行時確認)。review 経由でのドッグフード (本 track 自身の review) で確認

- [ ] Update .claude/agents/review-fix-lead.md: insert a new `## Scope-specific severity policy` section immediately before the existing `## Workflow` heading. The section must instruct the agent: when the main briefing contains a `## Scope-specific severity policy` section, the agent MUST Read the file listed there using the Read tool before starting the review; the referenced file defines which finding categories to report and which to skip for this scope; applying the wrong severity filter is the primary cause of over-long review loops (28-round history); always read the file fresh because the policy may have been updated between review sessions. The agent prompt must NOT carry a round budget / round count target — that belongs to the orchestrator pacing logic, not the reviewer severity filter. No automated test; dogfooded via the track's own review cycle.

## S005 — /track:review command doc 更新

.claude/commands/track/review.md の Step 2b の briefing 作成手順に、scope の briefing_file が Some なら CLI が自動で参照行を追加する旨の説明を追加する
Step 2b の briefing テンプレートには scope-specific severity policy 節を手動では書かない、composer が自動注入する、という責任分離を明記する
テスト: doc 変更のみ、CI の verify-arch-docs / verify-doc-links が通ることで検証

- [ ] Update .claude/commands/track/review.md Step 2b: clarify that when a scope has briefing_file configured in review-scope.json, the CLI wrapper (cargo make track-local-review / sotp review codex-local) automatically appends a `## Scope-specific severity policy` reference section to the main briefing. The briefing author (the review-fix-lead agent) must NOT manually add scope-specific severity policy wording — that responsibility belongs to the composer to avoid duplication. Update the scope list and briefing template section to mention plan-artifacts as a scope that benefits from this mechanism. Doc-only change; verified via cargo make verify-arch-docs / verify-doc-links.

## S006 — review-scope.json に plan-artifacts scope 追加

track/review-scope.json に plan-artifacts エントリを追加する
patterns: ["track/items/<track-id>/**", "knowledge/adr/**", "knowledge/research/**"] (T001 で loader の group pattern 展開が追加されたあと、この placeholder 付き最終形に切り替える。T006 の時点ではそれまで bootstrap として使っていた track/items/** を最終形に上書きする)
briefing_file: "track/review-prompts/plan-artifacts.md"
既存 scope 定義 (domain / usecase / infrastructure / cli / harness-policy) は一切変更しない
テスト: T002 の loader テストで plan-artifacts エントリが正しく parse されることを確認する統合テストを 1 件追加

- [ ] Bootstrap note: before this task lands, track/review-scope.json already carries plan-artifacts with a pragmatic pattern list ["track/items/**", "knowledge/adr/**", "knowledge/research/**"] (and no briefing_file) so the current review cycle works without relying on placeholder expansion that the pre-T001 loader does not perform. This task upgrades that bootstrap to the final form. Replace the plan-artifacts patterns with ["track/items/<track-id>/**", "knowledge/adr/**", "knowledge/research/**"] and add briefing_file: "track/review-prompts/plan-artifacts.md". Do not modify any existing scope definitions (domain, usecase, infrastructure, cli, harness-policy). The <track-id> placeholder must be expanded by ReviewScopeConfig::new for the current track per ADR D3 (T001 adds that expansion to the groups loop; verify T001 is merged before this task lands). review.json is already excluded via review_operational (applied before named-scope matching) so it does not leak into plan-artifacts. Files under knowledge/adr/** and knowledge/research/** are already in plan-artifacts under the bootstrap pattern; after this task they remain in plan-artifacts and continue to be reviewed under the correct severity policy. Integration test in infrastructure: load the updated review-scope.json and assert classify() places a sample track/items/<current>/spec.md, a sample knowledge/adr/xxxx.md, and a sample knowledge/research/xxxx.md into the plan-artifacts scope, and briefing_file_for_scope(plan-artifacts) returns Some("track/review-prompts/plan-artifacts.md").

## S007 — track/review-prompts/plan-artifacts.md 新規作成

track/review-prompts/ ディレクトリを新規作成し plan-artifacts.md を配置する
内容は knowledge/research/2026-04-18-0514-planner-review-scope-prompt-injection.md の Canonical Block 'plan-artifacts.md (initial body)' の全文 (severity policy: What to report / What NOT to report の 2 セクションのみ、round budget や round 数 cap は含めない — orchestrator pacing に属する)。ADR D3 の excerpt (§D3 抜粋) は 3-item の What NOT to report しか持たないため research note の canonical block が正規の source
research note canonical block の severity policy 仕様に完全準拠する
テスト: briefing_file の CI lint (broken link 検知) は Open Question Q3 として defer 済み。verify-doc-links は JSON briefing_file フィールドをスキャンしないため対象外。T008 ドッグフードで reviewer が Read ツールでファイルを読めることを実証

- [ ] Create directory track/review-prompts/ and file track/review-prompts/plan-artifacts.md. Content: the Plan Artifact Review severity policy text from the Canonical Block 'plan-artifacts.md (initial body)' in knowledge/research/2026-04-18-0514-planner-review-scope-prompt-injection.md — sections for What to report (factual error, contradiction, broken reference, infeasibility, timestamp inconsistency) and What NOT to report (wording nits, English/Japanese mix unless style rule violated, alternative design suggestions post-planning-gate, formatting preferences). The markdown must be self-contained: a reviewer who reads only this file must be able to apply the policy without consulting other docs. Do NOT include a round budget / round count target — such instructions belong to the orchestrator pacing logic, not to the reviewer's severity policy (embedding a round cap risks suppressing valid findings). Note: briefing_file CI lint (verify-doc-links checking JSON briefing_file entries) is deferred to Open Question Q3. The completion gate is T008 dogfood: reviewer must be able to Read the file via its Read tool.

## S008 — CI 通過 + ドッグフード

cargo make ci を全通過させる: fmt-check + clippy + nextest + test-doc + deny + python-lint + scripts-selftest + check-layers + verify-arch-docs + verify-doc-links
本 track 自身の review で plan-artifacts scope と briefing injection の end-to-end 動作を確認する (ドッグフード)
本 track 自身の review.json に plan-artifacts scope の zero_findings が記録されることを確認する

- [ ] Run cargo make ci and confirm all checks pass (fmt-check + clippy + nextest + test-doc + deny + python-lint + scripts-selftest + check-layers + verify-arch-docs + verify-doc-links). Then dogfood the feature by running /track:review on this track itself: the review cycle must classify track/items/review-scope-prompt-injection-2026-04-18/**, any modified knowledge/adr/** files, and any knowledge/research/** planner output for this track into the plan-artifacts scope, the CLI must inject a `## Scope-specific severity policy` reference to track/review-prompts/plan-artifacts.md, and the reviewer must report zero_findings for plan-artifacts after applying the severity policy. Record the dogfooding result (file counts per scope, review rounds, whether the severity policy was applied as expected) in verification.md.
