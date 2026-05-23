<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 26, yellow: 0, red: 0 }
---

# review-fix-lead の provider を選択可能にする (Claude デフォルト、Codex オプション)

## Goal

- [GO-01] review-fix-lead capability に Codex を追加の provider として追加し、テンプレートユーザーが agent-profiles.json の provider (および model) フィールドを変更するだけで Claude (デフォルト) と Codex (オプション) を切り替えられるようにする。他の capability (orchestrator / implementer / spec-designer 等) の provider は変更しない [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D1, knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D5]
- [GO-02] Codex provider を選択した場合、review-fix-lead が codex exec --sandbox workspace-write で起動される wrapper 経由で実行され、ローカル git 書き込みが構造的に防止された状態で agentic fix loop を自走できるようにする [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D2, knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D3]

## Scope

### In Scope
- [IN-01] agent-profiles.json への review-fix-lead capability エントリ新設: provider (claude / codex) と model フィールドを持ち、デフォルトは provider=claude で従来動作を維持する。Codex を選ぶ場合は provider=codex に変更する [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D5] [tasks: T001]
- [IN-02] /track:review の spawn 経路分岐: agent-profiles.json の review-fix-lead.provider を読み、provider==claude なら従来どおり Claude subagent (Agent tool) を起動し、provider==codex なら Codex 起動 wrapper (IN-03) を Bash 経由で起動する [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D5] [tasks: T002]
- [IN-03] Codex 起動 wrapper の新設: orchestrator から渡された briefing file path / scope 名 / scope file list を Codex prompt に inject し、codex exec --model <model> --sandbox workspace-write で Codex fixer を起動する。wrapper は Codex 終了後に return value (completed / blocked_cross_scope / failed) を parse して orchestrator に返す。wrapper の形式は bin/sotp サブコマンドか cargo make ラッパーのいずれか [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D2] [tasks: T003]
- [IN-04] 起動時 smoke-test の実装: wrapper はループ開始前に (1) workspace-write フラグが実際に渡されることをアサートする (設定ミスによる danger-full-access 起動を防ぐ)、(2) Codex CLI バージョンが .git 読み取り専用保護が維持されているバージョン範囲内であることを確認する [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D3] [tasks: T003]
- [IN-05] Codex 用 review-fix-lead briefing template の新設: .harness/briefings/review-fix-lead-codex.md として作成する。Claude tool 名 (Read / Grep / Glob / Edit) を Codex 慣用の操作 (cat 相当 / grep/rg / patch 相当) に翻訳し、cargo make track-local-review 等のシェルコマンドはそのまま引き継ぐ。共有部 (mission / contract / scope ownership / severity policy 参照) は Claude 版と文面を揃える [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D4] [tasks: T004]
- [IN-06] 既存 Claude review-fix-lead (.claude/agents/review-fix-lead.md) の残置: provider 切り替えは agent-profiles.json で行い、既存の Claude 版 agent 定義ファイルは削除しない [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D2] [tasks: T002, T005]

### Out of Scope
- [OS-01] orchestrator / implementer / spec-designer / type-designer / impl-planner / adr-editor の provider 変更: これらの capability は Claude Code のまま据え置く。Codex provider オプションの追加対象は review-fix-lead のみ [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D1]
- [OS-02] scope 境界違反の構造的防止: Codex fixer は workspace 内の任意ファイルを編集できる。scope 境界の遵守は briefing contract と orchestrator の pre-commit レビューに依存し、専用の構造的バリデーション実装は本 track では行わない (accepted risk) [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D3]
- [OS-03] ポスト実行の禁止 git 操作スキャン: workspace-write サンドボックスが .git をファイルシステムレベルで読み取り専用にするため、Codex は git 操作を物理的に実行できない。事後スキャンは冗長であり実装しない [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D3]
- [OS-04] 並列 review-fix-lead 実行時の cargo build lock 競合解消: 複数 scope を並列実行する際のビルドロック問題は本 track の scope 外 [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D2]
- [OS-05] shadow-mode fixer (Claude が driver、Codex が候補提示): 代替案 B として検討・却下済み。本 track では provider を排他選択する単純切替のみを実装する [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D1]

## Constraints
- [CN-01] Codex fixer の起動フラグは必ず --sandbox workspace-write とする。--sandbox danger-full-access および --dangerously-bypass-approvals-and-sandbox は禁止する。これらのフラグは .git 保護を解除し D3 の安全保証を無効化するため、wrapper が事前アサートで防止する [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D2, knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D3] [tasks: T003]
- [CN-02] Codex fixer に GITHUB_TOKEN / SSH_AUTH_SOCK / SSH 鍵ファイルパス等の push 手段を与えうるクレデンシャルを渡さない (credential isolation)。環境変数の除外に加え、SSH agent socket およびファイルシステム上の SSH 鍵 (~/.ssh 等) が fixer から利用できる状態にならないよう wrapper が保証する。ファイルシステムの .git 保護が退行した場合でも push 手段をフィクサーに与えない [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D3] [tasks: T003]
- [CN-03] Codex fixer はファイル編集のみ行い、コミットは行わない。コミットは trusted orchestrator (Claude Code、hook 対象) が既存の guarded wrapper (cargo make track-commit-message 等) 経由で実施する [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D3] [tasks: T003]
- [CN-04] provider=claude の場合、/track:review の動作は従来と完全に同一とする。既存の Claude review-fix-lead agent 定義 (.claude/agents/review-fix-lead.md) は変更せず、agent-profiles.json に review-fix-lead エントリを追加するだけでよい [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D5] [tasks: T001, T002]
- [CN-05] Codex 用 briefing template (.harness/briefings/review-fix-lead-codex.md) は Claude 版 (.claude/agents/review-fix-lead.md) をそのまま流用せず、Codex 用に並行版を新設する。共有部の文面は揃え、tool 指示部のみ provider 別に記述する [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D4] [tasks: T004]

## Acceptance Criteria
- [ ] [AC-01] agent-profiles.json に review-fix-lead capability エントリが存在し、provider フィールドを持つ。デフォルト状態 (provider=claude) で /track:review を実行すると、従来どおり .claude/agents/review-fix-lead.md 定義の Claude subagent が起動される。既存の provider=claude 動作に変化がない [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D5] [tasks: T001, T002]
- [ ] [AC-02] provider=codex に設定して /track:review を実行すると、Codex 起動 wrapper が呼ばれ、codex exec --sandbox workspace-write で Codex fixer が起動される。wrapper の起動時 smoke-test が (1) workspace-write フラグのアサート、(2) Codex CLI バージョン範囲チェックを実行する [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D2, knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D3] [tasks: T002, T003]
- [ ] [AC-03] Codex 起動 wrapper が --sandbox danger-full-access または --dangerously-bypass-approvals-and-sandbox フラグを渡しようとした場合、起動前アサートで失敗し Codex が起動されない [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D2, knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D3] [tasks: T003]
- [ ] [AC-04] .harness/briefings/review-fix-lead-codex.md が存在し、Claude tool 名 (Read / Grep / Glob / Edit) が Codex 慣用の操作に翻訳されている。mission / contract / scope ownership / severity policy 参照の共有部は Claude 版と文面が揃っている [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D4] [tasks: T004]
- [ ] [AC-05] Codex fixer が起動される際に GITHUB_TOKEN / SSH_AUTH_SOCK / SSH 鍵ファイルパス等の push 手段を与えうる環境変数がすべて除外されている。wrapper の実装上、credential isolation が保証されており、SSH agent socket (SSH_AUTH_SOCK) およびファイルシステム上の SSH 鍵 (~/.ssh 等) が fixer から利用できる状態にならない [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D3] [tasks: T003]
- [ ] [AC-06] Codex provider で実行した review-fix-lead が完了した後、git の index / HEAD に変更がない (Codex fixer がコミットを行っていない)。ファイル編集はワーキングツリーのみに留まり、orchestrator が track-commit-message wrapper 経由でコミットできる状態になっている [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D3] [tasks: T003]
- [ ] [AC-07] wrapper が Codex 終了後に completed / blocked_cross_scope / failed のいずれかの return value を parse して orchestrator に返す。orchestrator はこの値を使って次の action (commit / re-partition / エラーレポート) を決定できる [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D2] [tasks: T003]
- [ ] [AC-08] cargo make ci (fmt-check + clippy + nextest + deny + check-layers + verify-*) が pass する。provider=claude のデフォルト動作に変化がなく、既存テストにリグレッションが存在しない [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D5] [tasks: T001, T005]

## Related Conventions (Required Reading)
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/security.md#Sensitive Directories
- knowledge/conventions/enforce-by-mechanism.md#Rules
- .claude/rules/10-guardrails.md#Sandbox and Hook Coverage Warning (External Subprocesses)
- .claude/rules/04-coding-principles.md#No Panics in Library Code

## Signal Summary

### Stage 1: Spec Signals
🔵 26  🟡 0  🔴 0

