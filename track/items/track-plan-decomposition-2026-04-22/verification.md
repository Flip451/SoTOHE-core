# Verification — /track:plan 構造的分解 (Phase 0-3 独立 command + adr-editor capability + activate preflight 緩和 + subagent 責務拡大)

> **Track**: `track-plan-decomposition-2026-04-22`
> **ADR**: `knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md` (§展開フェーズ 4-6.5)
> **Scope**: T5 + T6 + T7 + T7.5 (ADR 展開フェーズ 4-6.5) + T8 相当の activate.rs preflight 緩和 (IN-10)

## 検証範囲

本 track の acceptance_criteria (AC-01..AC-22) に対応する手動 / 自動検証手順を以下に記録する。各 task (T001..T012) の実装完了時に結果を追記する。

## 手動検証手順

### T012 (activate.rs preflight 緩和)

1. `apps/cli/src/commands/track/activate.rs` の `activation_requires_clean_worktree` が `matches!(mode, BranchMode::Auto) && already_materialized && resume_allowed` になっている
2. 関連 rstest (`activation_resume_requires_clean_worktree`) が新セマンティクスに更新されている
3. `BranchMode::Auto` && `already_materialized=false` で `false` を返す unit test が存在する
4. `cargo make ci` が pass する (AC-12 / AC-20)
5. worktree に untracked files がある状態で `sotp track branch create '<track-id>'` が成功する (AC-19)

### T001 (/track:init 新規作成)

1. `.claude/commands/track/init.md` が存在する (AC-16)
2. command 定義本文に以下の step が含まれる:
   - track directory 作成 + metadata.json (branch=null 暫定) write
   - `sotp track branch create '<track-id>'` 呼び出し
   - metadata.json update (branch=track/<id>)
   - ADR 存在 pre-check (厳密モード)
   - identity schema binary 検証
3. state machine 遷移 / back-and-forth / max_retry / 事前承認 ceremony 文言が含まれない (AC-10 / AC-15)
4. /track:init <feature> 単独 invoke で上記を実行し gate 結果を返す (AC-01)
5. 注: AC-13 (schema validation ロジック不含検査) は CN-10 により /track:init を明示除外しているため、init.md は AC-13 の検査対象外

### T002 (/track:spec-design 新規作成)

1. `.claude/commands/track/spec-design.md` が存在する (AC-16)
2. command 定義本文が spec-designer subagent invocation + signal 評価結果受け取りのみ (AC-02 / AC-18)
3. sotp CLI 呼び出しが command 定義本文に含まれない (AC-22、grep 検証)
4. schema validation / signal 合成 / reference resolution / coverage / canonical block 検出のロジック記述が 0 件 (AC-13)
5. state machine 遷移ロジックが含まれない (AC-15)
6. 事前承認 ceremony 文言が含まれない (AC-10)

### T003 (/track:impl-plan 新規作成)

1. `.claude/commands/track/impl-plan.md` が存在する (AC-16)
2. command 定義本文が impl-planner subagent invocation + gate 結果受け取りのみ (AC-03 / AC-18)
3. sotp CLI 呼び出しが command 定義本文に含まれない (AC-22)
4. schema / signal / ref / coverage / canonical のロジック記述が 0 件 (AC-13)
5. state machine 遷移ロジックが含まれない (AC-15)
6. 事前承認 ceremony 文言が含まれない (AC-10)

### T004 (/track:type-design rename + trim)

1. `.claude/commands/track/type-design.md` が存在する (AC-16)、`.claude/commands/track/design.md` は削除されている
2. command 定義本文が type-designer subagent invocation + signal 評価結果受け取りのみ (AC-18 / AC-21)
3. baseline capture / types.json write / graph md / contract-map / type-signals md render / signal 評価がすべて type-designer 内部で実行される (AC-21)
4. sotp CLI 呼び出しが command 定義本文に含まれない (AC-22)
5. schema / signal / ref / coverage / canonical のロジック記述が 0 件 (AC-13)
6. state machine 遷移ロジックが含まれない (AC-15)
7. 事前承認 ceremony 文言が含まれない (AC-10)

### T005 (3 subagent agent definition 更新)

1. `.claude/agents/spec-designer.md` / `type-designer.md` / `impl-planner.md` の frontmatter tools に Write と Bash が追加されている (AC-17)
2. 各 subagent の Scope Ownership / Rules に (1) 対応 SSoT ファイルへの直接 Write 権限、(2) Bash (sotp CLI 専用) 使用権 が明記されている (AC-17)
3. 'This agent is read-only' / 'This agent is advisory' / 'orchestrator が transcribe する' の旧記述が削除または修正されている (AC-17)
4. Boundary table の typical trigger が新 command 名 (/track:spec-design / /track:type-design / /track:impl-plan) に更新されている
5. 各 subagent の内部 pipeline 記述 (Write + render + signal 評価) が Scope Ownership / Rules に明記されている (CN-12 / CN-13)

### T006 (adr-editor invocation contract 整合)

1. `.claude/agents/adr-editor.md` の Invocation contract セクションが /track:plan state machine の back-and-forth 呼び出し経路と矛盾しない (AC-11)
2. 呼び出し条件 (Phase 1 gate 🔴 かつ ADR commit 履歴あり → auto-invoke; 履歴なし → user pause) が明記 (AC-06 / AC-09)
3. briefing 必須項目 (🔴 発火 spec 要素 + 対象 ADR パス + working-tree-only 指示) が明記 (AC-06)
4. loop 中は commit しない制約が明記

### T007 (plan.md 構造スケルトン書き直し)

1. 冒頭に TaskCreate preamble が含まれる (Phase 0 / Phase 1 loop / Phase 2 loop / Phase 3 loop / 終端処理 + back-and-forth 逐次伝播を task として立てる明示指示) (AC-14)
2. Interim mode セクションが削除されている (grep で 0 件、AC-05)
3. phase invocation table が /track:init → /track:spec-design → /track:type-design → /track:impl-plan の順に書かれている
4. Sub-invocation details セクションが 'subagent が sotp CLI まで内部実行' 契約に更新されている
5. Writer ownership table から advisory/transcribe 文言が削除されている
6. plan.md 本文に schema validation / codec round-trip / signal 合成 / reference resolution / coverage 判定 / canonical block 検出のロジック記述が含まれていない (AC-13、grep で 0 件)

### T008 (plan.md back-and-forth loop body 書き直し)

1. Phase 1 loop block が記述されている (signal 結果読み取り + 🔴 時 adr-editor 分岐 + max_retry guard) (AC-04 / AC-06 / AC-09)
2. Phase 2 loop block が記述されている (🔴 時 /track:spec-design 再 invoke + escalation) (AC-04)
3. Phase 3 loop block が記述されている (ERROR 時同 phase 再 invoke) (AC-04)
4. 終端処理が記述されている (成功 / max_retry 超過いずれも ADR diff 提示 + accept/revert/manual/abort 判断) (AC-07)
5. max_retry positional integer 引数 (default 5、flag なし、`/track:plan 3` 形式) が記述されている (AC-08)
6. T008 で追加した loop body に schema validation / codec round-trip / signal 合成 / reference resolution / coverage 判定 / canonical block 検出のロジック記述が含まれていない (AC-13)

### T009 (SKILL.md 更新)

1. `.claude/skills/track-plan/SKILL.md` frontmatter description が新 command 名 (/track:init / /track:spec-design / /track:type-design / /track:impl-plan) を含む
2. /track:spec / /track:design の参照が /track:spec-design / /track:type-design に更新されている
3. SKILL.md 本体は plan.md へ defer する設計が維持されている (細部は plan.md)

### T010 (docs 更新)

1. `DEVELOPER_AI_WORKFLOW.md` に新 command 名が反映されている
2. `CLAUDE.md` に新 command 名が反映されている
3. `track/workflow.md` に新 command 名が反映されている
4. `knowledge/WORKFLOW.md` に新 command 名が反映されている
5. Phase 0-3 構造の大枠は維持され、差分の最小追記のみ (T001 PR #107 で既に記述された部分を尊重)

### T011 (cargo make ci 回帰ゲート)

1. `cargo make ci` が全項目 pass する (fmt-check + clippy + nextest + deny + check-layers + verify-* 一式) (AC-12)
2. 本 track の変更 (activate.rs 修正 + 新 command / agent / docs 追加) が regression を引き起こしていない

## 共通検証

1. `cargo make ci` が全通過する (AC-12)
2. 既存 track には遡及適用されず、既存 track の読み取りが壊れていない (OS-06)
3. `cargo make track-sync-views` で plan.md / spec.md / contract-map.md / registry.md が正常に render される
4. Phase 1 gate: spec → ADR signal 評価が `sotp verify plan-artifact-refs` で機械検証される
5. Phase 3 gate: task-coverage binary (coverage 強制 + referential integrity) が `sotp verify plan-artifact-refs` で PASSED

## Open Questions (OQ 定義)

本 spec の informal_grounds に登場する OQ ラベルの内容を以下に定義する。これらは ADR と spec の間の未解決差分を識別するために使用する。

| ID | 差分内容 | 状態 |
|----|----------|------|
| OQ-6 | コマンド名差分 (`/track:spec` → `/track:spec-design`, `/track:design` → `/track:type-design`) | CLOSED by 子 ADR 2026-04-22-0829 §D2 |
| OQ-7 | `/track:init` の branch create 責務が親 ADR §D0.0 で未言及 | CLOSED by 子 ADR 2026-04-22-0829 §D3 |
| OQ-8 | subagent write 権限付与が親 ADR §D4 で未言及 | CLOSED by 子 ADR 2026-04-22-0829 §D4 |

## 結果 / 未解決事項

全 T001-T012 の手動検証手順は各 task の commit + `cargo make ci` + review zero_findings (plan-artifacts / harness-policy / cli / domain / usecase 全 scope の full model 確認) により satisfy 済。上記 OQ table の通り未解決事項なし。

> Note: 本 track scope では verification.md を最小化 (結果 + verified_at のみ) とし、AC 充足の詳細は spec.json signals と review.json (zero_findings) を正とする。verification.md 自体の役割整理は `knowledge/conventions/workflow-ceremony-minimization.md` の思想に沿って follow-up track で ADR + workflow doc の整合を取る予定。

## verified_at

2026-04-22T12:00:00Z
