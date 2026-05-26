<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 41, yellow: 0, red: 0 }
---

# 完了済みトラック保護を frozen から現在ブランチ紐付きバリデーションへ置換 + plan-only / activate レーン削除

## Goal

- [GO-01] アーティファクトを書き換えるアクション（catalogue-spec-signals / type-signals / sync-views など）に対するトラック保護機構を、track status（done か否か）ベースの frozen ブロックから、現在の git ブランチに紐づくトラックかどうかを判定基準とするバリデーションへ置き換える。これにより、full-cycle 途中で status=done になったトラックでも現在ブランチが当該トラックブランチである限りアーティファクト更新が通るようにし、かつ現在ブランチに紐づかない完了済みトラックのアーティファクトは引き続き保護する [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1]
- [GO-02] 現在使われておらず保守コストになっている `/track:plan-only` と `/track:activate` の 2 段階ワークフローレーンを削除する。これらのコマンド定義、ソースモジュール、および専用の Makefile タスクを除去し、コードベースをシンプルに保つ [adr: knowledge/adr/2026-05-26-1123-remove-plan-only-activate-lane.md#D1]

## Scope

### In Scope
- [IN-01] アーティファクトを書き換えるすべてのアクション（catalogue-spec-signals / type-signals / sync-views、および同種の将来的なアクション）から、status=done/archived ベースの frozen ブロック判定ロジックを削除する [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [tasks: T001, T002, T003, T004]
- [IN-02] 削除した frozen ブロックの代わりに、「対象トラックのブランチ名（`track/<id>`）が現在の git ブランチと一致する場合のみアクションを許容する」というブランチベースのバリデーションを導入する。一致しない場合は明示的なエラーで拒否する（fail-closed） [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [tasks: T001, T002, T003]
- [IN-03] 背景 ADR `2026-04-15-1012-catalogue-active-guard-fix.md` が導入した catalogue active-track guard（`execute_type_signals` の status-based guard）を、今回のブランチベースバリデーションに置き換える。status ベースの frozen 判定ロジックを除去し、ブランチ紐付きバリデーションで同等の保護を実現する [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1, knowledge/adr/2026-04-15-1012-catalogue-active-guard-fix.md#2026-04-15-1012-catalogue-active-guard-fix_grandfathered] [tasks: T002, T003]
- [IN-04] sync-views コマンドが内部的に保持している `is_done_or_archived` ガード（`libs/infrastructure/src/track/render.rs` の文字列ベース `matches!` 判定）をブランチベースバリデーションに置き換える [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [tasks: T004]
- [IN-05] ブランチベースバリデーションに対するテストを追加する: 現在ブランチが当該トラックブランチと一致するケース（許容される）、一致しないケース（拒否される）の両方を検証する [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [tasks: T005]
- [IN-06] `.claude/commands/track/plan-only.md` と `.claude/commands/track/activate.md` のコマンド定義ファイルを削除する [adr: knowledge/adr/2026-05-26-1123-remove-plan-only-activate-lane.md#D1] [tasks: T007]
- [IN-07] `TrackCommand::Activate` variant（`apps/cli/src/commands/track/mod.rs`）と `execute_activate` / その専用ヘルパー関数群（`BranchMode::Auto` コードパス、activation resume marker、plan branch preflight など activation 固有の処理）を `apps/cli/src/commands/track/activate.rs` から削除する。`BranchAction::Switch` のルーティングは引き続き動作しなければならない（IN-08 を参照） [adr: knowledge/adr/2026-05-26-1123-remove-plan-only-activate-lane.md#D1] [tasks: T006]
- [IN-08] `track branch switch`（`BranchAction::Switch`）は削除後も動作しなければならない。現状は `execute_activate` を `BranchMode::Switch` で呼び出すことで実装されているため、activate 専用コードを除去した後は `BranchAction::Switch` 向けの独立した実装（既存ブランチへの `git switch` のみを行うシンプルなパス）を残す [adr: knowledge/adr/2026-05-26-1123-remove-plan-only-activate-lane.md#D1] [tasks: T006]
- [IN-09] `usecase::track_activation::ActivateTrackUseCase` および `libs/usecase/src/track_activation.rs` モジュールを削除する。また、activate ユースケースの依存モジュールである `libs/usecase/src/worktree_guard.rs`（`ensure_clean_worktree` / `parse_dirty_worktree_paths` / `validate_clean_worktree` 等の worktree guard 関数群を含む）も合わせて削除する。`BranchAction::Switch` の独立した実装はこれらのユースケースモジュールを必要としない [adr: knowledge/adr/2026-05-26-1123-remove-plan-only-activate-lane.md#D1] [tasks: T006]
- [IN-10] `Makefile.toml` の `track-activate` タスクと `track-plan-branch` タスクを削除する。`dispatch_track_activate` と `dispatch_track_plan_branch` ディスパッチャ（`apps/cli/src/commands/make.rs`）および対応する `MakeTask::TrackActivate` / `MakeTask::TrackPlanBranch` variant も削除する [adr: knowledge/adr/2026-05-26-1123-remove-plan-only-activate-lane.md#D1] [tasks: T006]
- [IN-11] `pr_workflow.rs` / `merge_gate.rs` / `task_completion.rs` / `track_resolution.rs` など共有ユースケースモジュールが持つ `plan/` プレフィックス対応のハンドリングロジック（`plan/<id>` ブランチ判定・分岐・関連エラーパス）を削除する。`/track:plan-only` と `/track:activate` を除去したことで `plan/<id>` ブランチを生成する手段が失われるため、到達不能になったコードパスを除去し、孤立した参照を残さない [adr: knowledge/adr/2026-05-26-1123-remove-plan-only-activate-lane.md#D1] [tasks: T008]
- [IN-12] `libs/domain/src/track_phase.rs` にある 2 つの enum から activate 関連 variant をそれぞれ削除する。(a) `NextCommand` enum から `NextCommand::ActivateTrack` variant を削除する。(b) `TrackPhase` enum から `TrackPhase::ReadyToActivate` variant を削除する。削除後は、これらの variant を参照しているすべてのマッチアームおよびパターンも合わせて除去する [adr: knowledge/adr/2026-05-26-1123-remove-plan-only-activate-lane.md#D1] [tasks: T008]
- [IN-13] `libs/domain/src/skill_compliance/mod.rs`（または同モジュール内）の skill-compliance 検出エントリのうち `/track:plan-only` と `/track:activate` に対応するものを削除する [adr: knowledge/adr/2026-05-26-1123-remove-plan-only-activate-lane.md#D1] [tasks: T008]
- [IN-14] `.claude/settings.json` の `permissions.allow` および `apps/cli/src/commands/verify/orchestra.rs`（または同等のオーケストラガード検証コード）から `track-activate` / `track-plan-branch` のパーミッションエントリを削除する [adr: knowledge/adr/2026-05-26-1123-remove-plan-only-activate-lane.md#D1] [tasks: T008]
- [IN-15] `git_cli` / `git_workflow` / `track_resolution` / `pr_workflow` / `render.rs` などのエラーメッセージ・レンダーテキスト、および `.claude/commands/track/plan.md` / `.claude/rules/07-dev-environment.md` 等のドキュメント内にある `/track:activate` / `/track:plan-only` / `track-activate` / `track-plan-branch` への参照を除去する [adr: knowledge/adr/2026-05-26-1123-remove-plan-only-activate-lane.md#D1] [tasks: T008]
- [IN-16] IN-11 〜 IN-15 の削除対象コードに紐づくテスト（`plan/` ブランチハンドリング、`NextCommand::ActivateTrack` および `TrackPhase::ReadyToActivate`、skill-compliance activate/plan-only エントリ、パーミッションエントリ、activate 参照文字列を検証するテスト）をすべて削除する [adr: knowledge/adr/2026-05-26-1123-remove-plan-only-activate-lane.md#D1] [tasks: T008]

### Out of Scope
- [OS-01] 複数トラックを同時に操作するユースケースへの対応: 「現在ブランチ = 単一トラック」という前提が崩れる複数トラック並行操作への対応は、本 track のスコープ外とする（ADR Reassess When に記載） [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1]
- [OS-02] CI 環境での detached HEAD やブランチ外からのバッチ処理への対応: ブランチに紐づかない文脈でのトラック操作が必要になる場合の判定基準拡張は本 track のスコープ外とする（ADR Reassess When に記載） [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1]
- [OS-03] rejected alternative A（frozen を残したまま done 判定に例外条件を追加）の実装: status ベース判定に条件分岐を重ねるアプローチは採用しない [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1]
- [OS-04] rejected alternative B（full-cycle の done マークを commit 後に遅延させる）の実装: done マークのタイミング変更は本 track のスコープ外とする。保護機構の置き換えのみを行う [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1]
- [OS-05] task の done マーク付けのタイミング変更: full-cycle が task を done マークするタイミングの変更は本 track のスコープ外とする（対症療法に留まるため、ADR で rejected alternative B として却下済み） [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1]

## Constraints
- [CN-01] ブランチベースのバリデーションは fail-closed で実装する: 対象トラックが現在のブランチに紐づかない場合、サイレントスキップや警告に留めず、明示的なエラーで拒否する [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [tasks: T002, T003, T004]
- [CN-02] バリデーションの配置はヘキサゴナルアーキテクチャの層依存方向に従う: ブランチ状態の読み取りはインフラ層の責務であり、バリデーションロジックは apps/cli 層または infrastructure 層に置き、domain 層を変更しない [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [conv: knowledge/conventions/hexagonal-architecture.md#Layer Dependencies] [tasks: T001]
- [CN-03] アーティファクトを書き換えるすべての経路（catalogue-spec-signals / type-signals / sync-views など）に対してブランチベースバリデーションを一貫して適用する。経路ごとに保護レベルが異なる非対称な状態を残さない [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [tasks: T002, T004]
- [CN-04] status フィールドを保護判定の主たる基準として使い続ける実装（status ベースの frozen 機構の温存）は行わない。判定基準を「現在ブランチとの紐付き」に一本化する [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [conv: knowledge/conventions/workflow-ceremony-minimization.md#Rules] [tasks: T001, T002, T003]
- [CN-05] plan-only / activate レーンを除去した後も `track branch switch`（`sotp track branch switch <id>`）は引き続き動作しなければならない。削除対象の activate 専用コードと switch パスを明確に分離し、switch の振る舞いにリグレッションを起こさない [adr: knowledge/adr/2026-05-26-1123-remove-plan-only-activate-lane.md#D1] [tasks: T006]

## Acceptance Criteria
- [ ] [AC-01] 現在ブランチが `track/<id>` である状態で、そのトラックの status が done であっても、catalogue-spec-signals / type-signals / sync-views が正常に実行されアーティファクトが更新される（full-cycle 途中の done マーク問題が解消される） [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [tasks: T002, T003, T004]
- [ ] [AC-02] 現在ブランチが `track/<id-A>` である状態で、別トラック `<id-B>` に対して catalogue-spec-signals / type-signals / sync-views を実行しようとすると、明示的なエラーで拒否される（`<id-B>` の status に関わらず） [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [tasks: T002, T003, T004]
- [ ] [AC-03] 「Completed tracks are frozen」という文言でブロックされていた操作（背景 ADR 2026-04-15-1012 の D1 ガードが出力していたメッセージ相当）が、ブランチベースバリデーションの導入後は発生しなくなる [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1, knowledge/adr/2026-04-15-1012-catalogue-active-guard-fix.md#2026-04-15-1012-catalogue-active-guard-fix_grandfathered] [tasks: T002, T003]
- [ ] [AC-04] `cargo make track-commit-message` の pre-commit フック内で実行される type signals / catalogue-spec-signals が、現在ブランチが当該トラックブランチである限り status=done のトラックでも「skipped (track is done — frozen)」とならずに実行される [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [tasks: T003]
- [ ] [AC-05] アーティファクトを書き換える複数の経路（catalogue-spec-signals / type-signals / sync-views）すべてで同じブランチベースバリデーションが適用される。いずれかの経路だけが旧 frozen ロジックを保持する非対称状態が存在しない [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [tasks: T002, T003, T004]
- [ ] [AC-06] `cargo make ci`（fmt-check + clippy + nextest + deny + check-layers + verify-*）が pass する [adr: knowledge/adr/2026-05-26-0518-active-track-write-guard.md#D1] [tasks: T005]
- [ ] [AC-07] `.claude/commands/track/plan-only.md` と `.claude/commands/track/activate.md` が存在しない。`/track:plan-only` と `/track:activate` コマンドは呼び出し不可能な状態になっている [adr: knowledge/adr/2026-05-26-1123-remove-plan-only-activate-lane.md#D1] [tasks: T007]
- [ ] [AC-08] `sotp track activate` サブコマンドが存在しない（`sotp track --help` のサブコマンド一覧に `activate` が表示されない）。`TrackCommand::Activate` variant が削除されている [adr: knowledge/adr/2026-05-26-1123-remove-plan-only-activate-lane.md#D1] [tasks: T006]
- [ ] [AC-09] `cargo make track-activate` および `cargo make track-plan-branch` タスクが存在しない（`Makefile.toml` から削除されている） [adr: knowledge/adr/2026-05-26-1123-remove-plan-only-activate-lane.md#D1] [tasks: T006]
- [ ] [AC-10] `usecase::track_activation` モジュールが存在しない（`libs/usecase/src/track_activation.rs` が削除されており、`lib.rs` からも参照されていない） [adr: knowledge/adr/2026-05-26-1123-remove-plan-only-activate-lane.md#D1] [tasks: T006]
- [ ] [AC-11] `sotp track branch switch <id>` が正常に動作する。既存の `track/<id>` ブランチへの切り替えができ、存在しないブランチに対してはエラーで拒否される [adr: knowledge/adr/2026-05-26-1123-remove-plan-only-activate-lane.md#D1] [tasks: T006]
- [ ] [AC-12] `sotp track branch create <id>` が正常に動作する（`track branch create` のリグレッションがない） [adr: knowledge/adr/2026-05-26-1123-remove-plan-only-activate-lane.md#D1] [tasks: T006]
- [ ] [AC-13] リポジトリ全体に対して `/track:activate` / `/track:plan-only` / `track-activate` / `track-plan-branch` / `execute_activate` / `track_activation` / `worktree_guard` をグレップしたとき、`knowledge/adr/**`（過去の決定記録）とこのトラック自身の成果物（`track/items/active-track-write-guard-2026-05-26/{spec.json,spec.md,impl-plan.json,plan.md}` など）以外でヒットがゼロである [adr: knowledge/adr/2026-05-26-1123-remove-plan-only-activate-lane.md#D1] [tasks: T008]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/source-attribution.md#Source Tag Types
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/05-testing.md#Test Structure

## Signal Summary

### Stage 1: Spec Signals
🔵 41  🟡 0  🔴 0

