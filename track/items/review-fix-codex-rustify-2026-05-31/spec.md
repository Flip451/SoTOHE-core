<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 36, yellow: 0, red: 0 }
---

# Codex review-fix-lead wrapper の Rust 化 — cargo make 流出ロジックを Rust 層へ移設

## Goal

- [GO-01] review-fix-lead の全オーケストレーションロジック（引数パース・smoke-test・credential isolation・プロンプト構築・`codex exec` 起動・sentinel パース・exit-code マッピング）を `Makefile.toml` のインライン bash から取り除き、reviewer と同じヘキサゴナル構造（usecase port + infrastructure adapter）に再実装することで、テスト容易性・層責務分離・shell 排除を同時に達成する [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D1, knowledge/adr/2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap.md#2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap_grandfathered]
- [GO-02] 外側 fixer の `codex exec --sandbox workspace-write` 起動時に `sandbox_workspace_write.writable_roots` に `CODEX_HOME`（`~/.codex`）を含め、かつ `sandbox_workspace_write.network_access=true` を設定することで、入れ子 reviewer の session 作成失敗（EROFS）を解消し、入れ子 agentic loop を維持したまま Codex review-fix-lead を実動作させる [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D2]
- [GO-03] 新実装を自己参照 dogfooding（その変更集合そのものに対して fixer を走らせる）で検証し、`agent-profiles.json` の `review-fix-lead.provider` を `codex` に切り替えることで、D2 の sandbox 修正が実負荷で有効であることを確認する [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D3]

## Scope

### In Scope
- [IN-01] usecase 層に fixer port（`Reviewer` と同様のアプリケーションサービス port。briefing / scope / scope-files / round-type / reviewer-model を入力に取り、`completed` / `blocked_cross_scope` / `failed` の結果を返す契約）と boundary DTO（command / output）を追加する [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D1] [tasks: T001]
- [IN-02] infrastructure 層に Codex fixer adapter を追加する。smoke-test（forbidden sandbox フラグ確認・codex CLI バージョン確認）・credential isolation（GITHUB_TOKEN 除外、SSH 鍵なし、GIT_SSH_COMMAND=/bin/false、一時 HOME 作成）・D2 の sandbox 設定（`writable_roots` に CODEX_HOME を含め `network_access=true`）・`codex exec` 起動・sentinel パース（`REVIEW_FIX_STATUS: completed/blocked_cross_scope/failed`）・exit-code マッピングをこの adapter に収める [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D1, knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D2] [tasks: T002]
- [IN-03] `apps/cli-composition` が `profiles.resolve_execution("review-fix-lead", round_type)` で provider を解決し adapter を wiring する（reviewer の `mod.rs` と同じ機構） [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D1, knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D1] [tasks: T003]
- [IN-04] `apps/cli` に clap サブコマンドを追加し、現行の 7 フラグ（`--scope` / `--briefing-file` / `--track-id` / `--round-type` / `--reviewer-model` / `--model` / `--scope-files`）を受け付ける。このサブコマンドは `apps/cli-composition` の fixer wiring メソッドを呼ぶだけの薄い glue になる [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D1, knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D1] [tasks: T004]
- [IN-05] `Makefile.toml` の `track-local-review-fix-codex` タスクをインライン bash から `bin/sotp make track-local-review-fix-codex "$@"` の thin passthrough に置き換える [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D1, knowledge/adr/2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap.md#2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap_grandfathered] [tasks: T005]
- [IN-06] 外側 fixer adapter が `codex exec` を起動する際に `sandbox_workspace_write.writable_roots` に `CODEX_HOME`（`~/.codex`）を含め、かつ `sandbox_workspace_write.network_access=true` を設定する。これにより入れ子 reviewer が `~/.codex/sessions` に書き込めるようになり EROFS エラーが解消する [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D2] [tasks: T002]
- [IN-07] `agent-profiles.json` の `review-fix-lead` capability に `provider: codex` を設定し、dogfooding（このトラック変更集合そのものへの fixer 実行）で実負荷検証する。dogfooding が安定すれば `codex` 固定を維持してよい [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D3, knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D5] [tasks: T006]
- [IN-08] 現行 bash 実装が提供している credential isolation の安全モデル（`.git` read-only + GITHUB_TOKEN 除外 + SSH 系除外）を Rust adapter でも同等に維持する。`network_access=true` は push 防止を弱めない（ADR D2 §安全性） [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D2, knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D3] [tasks: T002]

### Out of Scope
- [OS-01] `apps/cli-composition` にのみ閉じた実装（port / adapter を作らない Rejected Alternative A）: reviewer がヘキサゴナル構造を持つのに fixer だけ持たないのは非対称であり、mock によるユニットテストもできない。ADR は hexagonal-port-adapter を採用した [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D1]
- [OS-02] Rust 側で agentic loop を駆動し reviewer を host レベルで別個に呼ぶ de-nest 方式（Rejected Alternative B）: review → fix の連続コンテキストが失われる。session 失敗は D2 の sandbox 設定で入れ子のまま解消できることが実演済みのため不要 [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D2]
- [OS-03] 並列固有の per-scope 包括的リソース分離（Rejected Alternative C）: 再現調査により、入れ子 reviewer の session 作成失敗は並列固有ではなく（単独でも失敗）、真因は session ディレクトリが外側 sandbox の writable root 外であることと確定。包括的分離は誤診に基づく過剰実装になる [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D2]
- [OS-04] dogfooding を経ずに provider を Codex に固定する（Rejected Alternative D）: Codex rfl の安定性は自己参照 dogfooding で実負荷検証してから維持を判断する [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D3]
- [OS-05] fixer port / adapter の具体的な型形状・メソッドシグネチャ・kind 選択（newtype / typestate 等）: これらは Phase 2（type-design）が確定する。ADR D1 は「port / adapter / DTO の正確な型形状・メソッド・kind は type-design で確定する」と明示している [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D1]
- [OS-06] codex の `failed to record rollout items` telemetry エラーへの対処: ADR Neutral に「非致命的事象であり本 ADR の対象外」と明記されている [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D2]
- [OS-07] orchestrator / implementer / spec-designer / type-designer / impl-planner / adr-editor を Codex に置き換える: ADR 2026-05-23-1848 D1 で review-fix-lead 単体のみが Codex 化の対象と明示されている [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D1]
- [OS-08] Claude 版 review-fix-lead（`.claude/agents/review-fix-lead.md`）の削除: ADR 2026-05-23-1848 D2 で「既存 Claude review-fix-lead は引き続き残置し、agent-profiles.json の設定で provider を切り替えられるようにする」と明示されている [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D2]

## Constraints
- [CN-01] fixer port は usecase 層に置く。外部プロセス起動・実行環境構築・`codex exec` 呼び出しは infrastructure adapter の責務であり usecase 層に直接書いてはならない（`std::process::*` は usecase 層で禁止） [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D1] [conv: knowledge/conventions/hexagonal-architecture.md#Usecase Layer Purity Rules]
- [CN-02] fixer の clap サブコマンドは `apps/cli-composition` の wiring メソッド経由でのみ usecase にアクセスする（cli は usecase / infrastructure を直接 import しない） [adr: knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D1, knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D3]
- [CN-03] 外側 fixer の sandbox は `workspace-write` に固定する。`danger-full-access` および `--dangerously-bypass-approvals-and-sandbox` は禁止（`.git` の read-only 保護が解除されるため）。禁止フラグが環境変数等から渡されていないかを adapter の smoke-test で事前確認する [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D2, knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D3]
- [CN-04] adapter は `codex exec` 起動前に2つの smoke-test を実施する: (1) forbidden sandbox フラグが環境変数に存在しないことの確認、(2) codex CLI のバージョンが検証済み範囲（>= 0.115.0、< 1.0.0）内であることの確認。いずれかの失敗は exit code 2 で即時終了する [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D3]
- [CN-05] credential isolation として、adapter は `codex exec` を起動する際に GITHUB_TOKEN / SSH_AUTH_SOCK / GIT_SSH / GIT_SSH_COMMAND（/bin/false に上書き）/ SSH_CONNECTION / SSH_CLIENT を渡さず、HOME を SSH 鍵を持たない一時ディレクトリに置き換える。CODEX_HOME（`~/.codex`）は model 推論認証のために明示的に渡す [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D3]
- [CN-06] `Makefile.toml` の `track-local-review-fix-codex` タスクは `bin/sotp make track-local-review-fix-codex "$@"` の thin passthrough のみとし、インライン bash ロジックを持たない（`sotp make Dispatch` 規約） [adr: knowledge/adr/2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap.md#2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap_grandfathered]
- [CN-07] `cargo make ci`（fmt-check + clippy + nextest + deny + check-layers + verify-*）が pass する状態を各コミット時に維持する。CI を壊す中間コミットをトラックブランチ上に残さない [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D1]

## Acceptance Criteria
- [ ] [AC-01] `libs/usecase/src/` 配下に fixer port trait と boundary DTO（command / output / error 型）が存在し、それらが標準ライブラリプリミティブ（String / PathBuf 等）のみを公開面に出し、`domain` / `infrastructure` 型を漏らしていない [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D1] [tasks: T001]
- [ ] [AC-02] `libs/infrastructure/src/` 配下に Codex fixer adapter（`CodexReviewFixRunner` 相当）が存在し、fixer port trait を実装している。adapter が smoke-test・credential isolation・D2 の sandbox 設定（`writable_roots` に CODEX_HOME 含む + `network_access=true`）・sentinel パース・exit-code マッピングを実装している [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D1, knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D2] [tasks: T002]
- [ ] [AC-03] `apps/cli-composition/src/` 配下に fixer の wiring コードが存在し、`profiles.resolve_execution("review-fix-lead", round_type)` によって provider を解決して adapter を選択する [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D1, knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D1] [tasks: T003]
- [ ] [AC-04] `apps/cli/src/` に fixer clap サブコマンドが存在し、7 フラグ（`--scope` / `--briefing-file` / `--track-id` / `--round-type` / `--reviewer-model` / `--model` / `--scope-files`）を受け付け、`apps/cli-composition` の wiring メソッドを呼ぶだけの薄い glue になっている。`apps/cli/src/` 内に `use infrastructure::` / `use usecase::` の直接 import が存在しない [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D1, knowledge/adr/2026-04-30-0848-cli-via-usecase-only.md#D1] [tasks: T004]
- [ ] [AC-05] `Makefile.toml` の `track-local-review-fix-codex` タスクがインライン bash を持たず、`bin/sotp make track-local-review-fix-codex "$@"` のみで構成されている [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D1] [tasks: T005]
- [ ] [AC-06] fixer port の unit テストが存在し、mock adapter（`completed` / `blocked_cross_scope` / `failed` それぞれのシナリオ）で port contract を検証している [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D1] [tasks: T001]
- [ ] [AC-07] adapter の smoke-test が forbidden sandbox フラグ（`danger-full-access` / `dangerously-bypass-approvals-and-sandbox`）を環境変数から検出したとき、exit code 2 で終了することを確認するテストが存在する [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D3] [tasks: T002]
- [ ] [AC-08] adapter の sentinel パース処理が `REVIEW_FIX_STATUS: completed` / `blocked_cross_scope` / `failed` のフルライン一致のみを受理し、sentinel を含む散文に埋め込まれた偽 sentinel を除外することを確認するテストが存在する [adr: knowledge/adr/2026-05-23-1848-review-fix-lead-codex-migration.md#D2] [tasks: T002]
- [ ] [AC-09] `agent-profiles.json` の `review-fix-lead` capability に `provider: codex` が設定されており、dogfooding 実行の記録（完走確認または安定しない場合の `claude` への差し戻し）が本トラックの実装完了時に観測されている [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D3] [tasks: T006]
- [ ] [AC-10] `cargo make ci`（fmt-check + clippy + nextest + deny + check-layers + verify-*）が pass する [adr: knowledge/adr/2026-05-31-0542-review-fix-codex-hexagonal-nested-session.md#D1] [tasks: T001, T002, T003, T004, T005, T006]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md#Port Placement Rules
- knowledge/conventions/hexagonal-architecture.md#Adapter Rules
- knowledge/conventions/hexagonal-architecture.md#Usecase Layer Purity Rules
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/security.md#Symlink Rejection in Infrastructure Adapters
- .claude/rules/04-coding-principles.md#Trait-Based Abstraction (Hexagonal Architecture)
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/05-testing.md#Test Structure

## Signal Summary

### Stage 1: Spec Signals
🔵 36  🟡 0  🔴 0

