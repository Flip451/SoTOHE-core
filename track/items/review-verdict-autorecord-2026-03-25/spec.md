<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0.0"
---

# RVW-10/11 Review verdict auto-record + diff scope enforcement

## Goal

レビュー verdict の改竄を構造的に防止し（RVW-10）、変更範囲外ファイルの finding を構造的に除外する（RVW-11）。
codex-local コマンドが verdict 抽出後にプロセス内で record-round を直接呼び出す --auto-record モードを追加し、オーケストレータを verdict→record パスから除外する。
diff ファイルリストで findings をフィルタし、正規化された repo-relative パスで exact match 比較を行う。正規化不能なパスは fail-closed で in-scope 扱いする。

## Scope

### In Scope
- usecase 層に DiffScope, RepoRelativePath, DiffScopeProvider port, scope filtering 関数を追加 [source: knowledge/strategy/TODO.md §RVW-11, Codex planner analysis 2026-03-25] [tasks: T001]
- record_round_typed() — parsed domain 型を直接受け取る typed usecase エントリポイント追加 [source: Codex planner analysis 2026-03-25] [tasks: T002]
- GitDiffScopeProvider infrastructure adapter (merge-base diff, renames, deletions, untracked files) [source: Codex planner analysis 2026-03-25] [tasks: T003]
- CodexLocalArgs 拡張: --auto-record, --track-id, --round-type, --group, --expected-groups, --items-dir, --diff-base [source: knowledge/strategy/TODO.md §RVW-10] [tasks: T004]
- auto-record 実行フロー: verdict 抽出 → scope filter → concerns 抽出 → record_round_typed 内部呼出 [source: knowledge/strategy/TODO.md §RVW-10, feedback — verdict falsification incident 2026-03-24] [tasks: T005]
- Makefile.toml + orchestrator command (review.md) の --auto-record --diff-base 対応 [source: inference — integration requirement] [tasks: T006]

### Out of Scope
- ReviewFinding / ReviewFinalPayload の domain 層移動（usecase 配置が正しいと確認済み） [source: Codex planner analysis 2026-03-25]
- suffix match によるパス照合（monorepo で mod.rs, lib.rs 等が曖昧になるため不採用） [source: Codex planner analysis 2026-03-25]
- review.json 分離 (RVW-03) — 別トラック [source: knowledge/adr/2026-03-24-1200-review-state-trust-model.md]
- auto mode 統合 — 別フェーズ [source: knowledge/strategy/TODO-PLAN.md §Phase 5]

## Constraints
- 新規ロジックは Rust で実装（Python 禁止） [source: feedback — Rust-first policy]
- TDD: テストを先に書く (Red → Green → Refactor) [source: convention — .claude/rules/05-testing.md]
- domain 層に serde を追加しない（既存規約維持） [source: convention — libs/domain/src/review/types.rs L247]
- --auto-record 無効時は既存動作を完全保持（後方互換） [source: inference — breaking change prevention]
- auto-record 引数は Codex サブプロセス起動前に検証（fail fast） [source: Codex planner analysis 2026-03-25]

## Domain States

| State | Description |
|-------|-------------|
| FindingScopeClass | InScope | OutOfScope | UnknownPath — finding のスコープ分類結果 |
| DiffScope | BTreeSet<RepoRelativePath> — diff に含まれるファイルパスの集合 |
| ScopeFilteredPayload | adjusted_payload + out_of_scope + unknown_path_count — フィルタ適用後の分割結果 |

## Acceptance Criteria
- [ ] sotp review codex-local --auto-record が verdict 抽出後に record-round を内部呼出しし、オーケストレータが verdict を経由しない [source: knowledge/strategy/TODO.md §RVW-10] [tasks: T004, T005]
- [ ] 変更範囲外ファイルの finding が out_of_scope として分離され、adjusted verdict が正しく計算される [source: knowledge/strategy/TODO.md §RVW-11] [tasks: T001]
- [ ] file: null の finding は in-scope として扱われる [source: Codex planner analysis 2026-03-25] [tasks: T001]
- [ ] 正規化不能パスの finding は in-scope + unknown_path_count に計上される（fail-closed） [source: Codex planner analysis 2026-03-25] [tasks: T001]
- [ ] --auto-record なしの既存呼出が動作変更なく通る [source: inference — backward compatibility] [tasks: T004]
- [ ] escalation block 時に exit code 3 が返り、auto-record がスキップされる [source: inference — escalation contract consistency with record-round] [tasks: T005]
- [ ] 各新規 public API (scope.rs, record_round_typed, GitDiffScopeProvider, auto-record flow) に TDD でハッピーパス + エラーケースのテストがある [source: convention — .claude/rules/05-testing.md] [tasks: T001, T002, T003, T004, T005]
- [ ] cargo make ci が通る [source: convention — .claude/rules/07-dev-environment.md] [tasks: T001, T002, T003, T004, T005, T006]

## Related Conventions (Required Reading)
- project-docs/conventions/hexagonal-architecture.md
- project-docs/conventions/source-attribution.md
- project-docs/conventions/task-completion-flow.md

