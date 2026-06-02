<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.2"
signals: { blue: 40, yellow: 0, red: 0 }
---

# CI 所要時間の短縮(キャッシュ戦略)+ review/commit ゲートのカタログ未生成耐性 + --lenient / --force 実行経路の撤去 + review fixer の引数最小化(--scope-files / --reviewer-model 廃止)

## Goal

- [GO-01] 重量級ネイティブ依存追加後に約 14〜16 分に伸びた CI の所要時間を、ソースコード（機能実装・依存構成）を変更せず、キャッシュ戦略の見直しのみで短縮する。原因の特定と具体的な手段の選定は track 内の試行錯誤で行い、本 spec は結果・制約レベルでの要件のみ記述する [adr: knowledge/adr/2026-06-01-0336-ci-shorten-cache-strategy-only.md#D1]
- [GO-02] `track-active-gate` のシグナル再生成ステップ（`type-signals` 等）が、評価対象の層カタログが存在しない場合に no-op + warning で skip し非ゼロ終了しないようにする。これにより `track-local-review` と `track-commit-message` が Phase 0 / 1 の段階（型カタログ未生成）でも成功し、init 直後の ADR baseline commit フローが通る [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1]
- [GO-03] `type-signals` の `--lenient` フラグ（gate-vs-direct の区別）と `baseline-capture` の `--force` フラグ（baseline 上書き）を撤去し、両コマンドの実行経路を単純で安全な一本道にする。`type-signals` は views sync / catalogue-spec-signals と同様に全呼び出し経路でカタログ不在を無条件 skip し、`baseline-capture` は常に冪等（既存 baseline は保持）とする [adr: knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md#D1, knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md#D2]
- [GO-04] review fixer（`sotp review fix-local`）が orchestrator から受け取る引数を `--scope` / `--briefing-file` / `--track-id` / `--round-type` の4つに最小化する。`--scope-files`（修正対象ファイル一覧）は fixer skill 内の正規コマンド（`bin/sotp review files --scope <scope>`）による自己解決に切り替え、`--reviewer-model`（入れ子 reviewer の model 指定）は reviewer 起動コマンドの自己解決（`agent-profiles.json` から round-type 別に取得）に任せる。これにより orchestrator 側のスコープ分類の複製と、`.github` を含むファイルパスが `block-direct-git-ops` フックに当たる footgun を解消する [adr: knowledge/adr/2026-06-01-2300-review-fixer-self-resolve-scope-files.md#D1, knowledge/adr/2026-06-01-2300-review-fixer-self-resolve-scope-files.md#D2, knowledge/adr/2026-06-01-2300-review-fixer-self-resolve-scope-files.md#D3]

## Scope

### In Scope
- [IN-01] CI キャッシュ設定ファイル（`.github/workflows/` 等のワークフロー定義、`Makefile.toml` のキャッシュ関連設定、Docker layer キャッシュ設定など）のうちキャッシュ戦略に関わる部分を調整する対象とする [adr: knowledge/adr/2026-06-01-0336-ci-shorten-cache-strategy-only.md#D1] [tasks: T002]
- [IN-02] `type-signals`（および同じく未生成の上流成果物を読む他のシグナルステップ）を、対象層のカタログファイルが存在しない場合に no-op + warning で skip し非ゼロ終了しないよう修正する対象とする。この skip 挙動は `views sync` / `catalogue-spec-signals` の既存の無条件 absent-skip に揃え、gate 経由呼び出しと直接呼び出しを区別しない（`--lenient` フラグ等の呼び出し経路依存分岐は撤去する） [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1, knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md#D1] [tasks: T003, T005]
- [IN-03] カタログが存在する層については従来どおりシグナルを評価し、🔴 は引き続きゲートを block する。skip は「カタログファイルが存在しない層」に限定し、カタログがある層の評価を省く fail-open は作らない [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1] [tasks: T003]
- [IN-04] `baseline-capture` の `--force` 実行経路（既存 baseline の上書き）を撤去し、baseline 取得を常に冪等な操作にする対象とする。既存 baseline が存在する場合は取得をスキップし、再取得が必要な場合は「baseline ファイルを削除してから capture を再実行する」運用に倒す [adr: knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md#D2] [tasks: T006]
- [IN-05] review-fix-lead skill（`review-fix-lead.md` 等）に、正規スコープ分類コマンド（`bin/sotp review files --scope <scope>`）を skill 自身が実行して修正対象ファイル一覧（modification boundary）を得る指示を書く。これにより `--scope-files` の外部受領なしで境界を解決できるようにする [adr: knowledge/adr/2026-06-01-2300-review-fixer-self-resolve-scope-files.md#D1] [tasks: T008]
- [IN-06] `sotp review fix-local` の `--scope-files` フラグと `EmptyScopeFiles` guard（空拒否チェック）を撤去する。`/track:review` skill からスコープファイル一覧を導出・付与する指示を削除し、fixer 起動コマンドから `--scope-files` を外す [adr: knowledge/adr/2026-06-01-2300-review-fixer-self-resolve-scope-files.md#D1, knowledge/adr/2026-06-01-2300-review-fixer-self-resolve-scope-files.md#D2] [tasks: T007, T008]
- [IN-07] `sotp review fix-local` の `--reviewer-model` フラグを撤去する。fixer が入れ子で起動する reviewer コマンド（`cargo make track-local-review` = `sotp review local`）は `agent-profiles.json` の `reviewer` capability から round-type 別に model を自己解決するため、外部から model を渡す必要はなく、`--reviewer-model` の受け渡し指示を skill / skill 起動コマンドの両側から取り除く [adr: knowledge/adr/2026-06-01-2300-review-fixer-self-resolve-scope-files.md#D3] [tasks: T007, T008]

### Out of Scope
- [OS-01] CI キャッシュ短縮作業におけるソースコード（機能実装・依存構成）の変更: ADR D1 が「ソースコード変更なし、キャッシュ戦略の見直しのみで対応する」と明示している。この制約は ADR 2026-06-01-0336 / T002 のキャッシュ戦略変更に限定し、ADR 2026-06-01-0406 / T003 の review gate 実装修正および ADR 2026-06-01-1206 の --lenient / --force 撤去を禁じない [adr: knowledge/adr/2026-06-01-0336-ci-shorten-cache-strategy-only.md#D1]
- [OS-02] CI 所要時間が伸びた具体的な原因の確定: ADR が「所要時間が伸びた具体的な原因は未確定であり、調査・試行錯誤の中で変わりうる」と明示している。spec は原因を断定しない [adr: knowledge/adr/2026-06-01-0336-ci-shorten-cache-strategy-only.md#D1]
- [OS-03] 使用するキャッシュ機構の事前確定（例: sccache layer 設定変更・GitHub Actions cache アクション切り替えなど）: 具体的な手段の選定は track での試行錯誤に委ねる。ADR は「具体的な手段の選定は track での試行錯誤を通じて行い、本 ADR では確定させない」と明示している [adr: knowledge/adr/2026-06-01-0336-ci-shorten-cache-strategy-only.md#D1]
- [OS-04] `track-local-review` / `track-commit-message` から `track-active-gate` 依存を丸ごと外す: カタログが存在する後続フェーズでもシグナル再生成が走らなくなり、reviewer / commit が古いシグナル状態を見る(hash mismatch / fail-open)。ADR Rejected Alternative A として記録されている [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1]
- [OS-05] Phase 0 専用の別レビュー経路の新設: `views sync` と同じ「入力不在なら skip」という一様なルールで足りるため、フェーズ別の経路を増やすのは不要な複雑化。ADR Rejected Alternative B として記録されている [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1]
- [OS-06] type-signals / baseline-capture の型契約変更（`MissingCataloguePolicy` enum 削除・`TypeSignalsExecutorPort::evaluate_layer` の policy 引数削除・`TypeSignalsRequest.lenient` 削除、および `RustdocBaselineCapturePort::capture` の force 引数削除・`BaselineCaptureRequest.force` 削除・`force_capture_rustdoc_baseline_for_layer` 削除等）のメソッドシグネチャ・具体的な型の選択: これらは Phase 2（type-design）が catalogue で宣言する契約レベルの事項であり、本 spec は振る舞い契約のみを記述する [adr: knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md#D1, knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md#D2]
- [OS-07] `catalogue-spec-signals` 側への `--lenient` フラグ追加（対称化）: 余分な機構をさらに増やす方向であり、ADR Rejected Alternative A として記録されている。本トラックは両コマンドの挙動を「フラグなし・無条件 absent-skip」で統一する方向をとる [adr: knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md#D1]
- [OS-08] `baseline-capture --force` の運用注意による存続: 危険な上書き経路をドキュメントだけで防ぐ案は ADR Rejected Alternative C として却下されている。本トラックは型・CLI レベルで経路自体を削除する [adr: knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md#D2]
- [OS-09] `--scope-files` の optional 化（未指定時のみ自己解決する二重経路）: フラグを残すと orchestrator 経由と skill 自己解決の二重経路が残り、footgun が完全には消えず境界解決の出所が一本化しない。ADR Rejected Alternative B として却下されている [adr: knowledge/adr/2026-06-01-2300-review-fixer-self-resolve-scope-files.md#D1] [tasks: T007]
- [OS-10] reviewer model の型契約変更（cli / cli-composition / usecase / infrastructure の複数層にわたる public 型のフィールド削除）のメソッドシグネチャ・具体的な型の選択: `reviewer_model` フィールドを持つ `FixLocalArgs` / `RunReviewFixLocalInput` / `RunReviewFixCommand` および `review_fix_runner` の prompt / コマンド組み立て部分の具体的な型・シグネチャ変更は Phase 2（type-design）が catalogue で宣言する契約レベルの事項であり、本 spec は振る舞い契約のみを記述する [adr: knowledge/adr/2026-06-01-2300-review-fixer-self-resolve-scope-files.md#D3] [tasks: T007]

## Constraints
- [CN-01] CI 設定の変更はキャッシュ戦略に限定する。ソースコード（`libs/` / `apps/` 配下の Rust ソース）・`Cargo.toml` / `Cargo.lock`（依存構成）・機能フラグ・スクリプトのロジックには手を加えない [adr: knowledge/adr/2026-06-01-0336-ci-shorten-cache-strategy-only.md#D1] [tasks: T002]
- [CN-02] シグナルステップの skip は「入力カタログファイルが存在しない層」に限定する。カタログが存在する層はすべて従来どおり評価し、🔴 は引き続きゲートを block する（fail-open を作らない） [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1] [tasks: T003]
- [CN-03] skip 時の挙動は `views sync` / `catalogue-spec-signals` の既存実装（カタログ不在を warning で skip・ハードエラーなし）と一貫させる。gate 経由・直接呼び出しを問わず、`type-signals`（および同種のシグナルステップ）は欠損カタログに対して同じ absent-skip の挙動を持ち、呼び出し経路によって振る舞いが変わらない（`--lenient` フラグによる gate-vs-direct 区別は存在しない） [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1, knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md#D1] [tasks: T003, T005]
- [CN-04] `cargo make ci`（fmt-check + clippy + nextest + deny + check-layers + verify-*）が pass する状態を各コミット時に維持する [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1] [tasks: T002, T003, T004, T005, T006, T007, T008]
- [CN-05] `baseline-capture` は常に冪等とする。既存 baseline ファイルが存在する場合は取得をスキップし上書きしない。baseline の再取得が必要な場合は「baseline ファイルを削除してから capture を再実行する」2 手順の運用とする。`--force` による 1 コマンド上書きは提供しない [adr: knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md#D2] [tasks: T006]
- [CN-06] スコープ境界の解決は skill 内の正規コマンド（`bin/sotp review files --scope <scope>`）による一本化とする。orchestrator がファイルパスをコマンド文字列に乗せる経路（`--scope-files` 経由）を残さない。Rust 側に新たなスコープ分類 port を追加するのではなく、フラグと guard を撤去するだけ（純減）の変更とする [adr: knowledge/adr/2026-06-01-2300-review-fixer-self-resolve-scope-files.md#D1] [tasks: T007, T008]
- [CN-07] reviewer 起動コマンド（`sotp review local`）は `agent-profiles.json` の `reviewer` capability から model を round-type 別に自己解決する（fast round は `fast_model`、final round は `model`）。fixer は reviewer の model を外部から受け取らず、reviewer 起動コマンドに `--model` を明示的に渡さない [adr: knowledge/adr/2026-06-01-2300-review-fixer-self-resolve-scope-files.md#D3] [tasks: T007, T008]

## Acceptance Criteria
- [ ] [AC-01] `track-local-review` と `track-commit-message` が、型カタログ（例: `domain-types.json`）が存在しない Phase 0 / 1 の状態でゼロ終了する。`type-signals evaluation failed for layer 'domain': failed to read catalogue ...` のようなハードエラーが出ない [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1] [tasks: T003]
- [ ] [AC-02] カタログが存在しない層を skip するとき、warning メッセージが標準エラーまたは標準出力に出力される（silent no-op でなく、skip した旨が観測できる） [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1] [tasks: T003]
- [ ] [AC-03] 型カタログが存在する層（Phase 2 以降）では `type-signals` がシグナルを従来どおり評価し、🔴 の場合にゲートが非ゼロ終了する（skip による fail-open が発生していない） [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1] [tasks: T003]
- [ ] [AC-04] キャッシュ戦略の調整後、CI の所要時間が従来（重量級ネイティブ依存追加前）の水準に近づく、または明確に短縮される。ソースコード（`libs/` / `apps/` の Rust ソース・`Cargo.toml` / `Cargo.lock`）に変更がない [adr: knowledge/adr/2026-06-01-0336-ci-shorten-cache-strategy-only.md#D1] [tasks: T001, T002]
- [ ] [AC-05] `cargo make ci` が pass する（fmt-check + clippy + nextest + deny + check-layers + verify-* の全ステップ） [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1] [tasks: T004]
- [ ] [AC-06] `type-signals` をユーザーが直接（gate 経由でなく）呼び出したとき、カタログ不在の層は gate 経由と同じ absent-skip の挙動をとり、エラーにならない（`--lenient` フラグなしの状態で gate/direct の区別が消えている） [adr: knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md#D1] [tasks: T005]
- [ ] [AC-07] `baseline-capture` を既存 baseline が存在する状態で実行しても、既存 baseline が上書きされない（冪等に skip される） [adr: knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md#D2] [tasks: T006]
- [ ] [AC-08] `baseline-capture --source-workspace` が引き続き利用可能であり、main の git worktree から baseline を取得する正規の運用が維持される [adr: knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md#D2] [tasks: T006]
- [ ] [AC-09] `sotp review fix-local` を `--scope-files` なしで呼び出せる。`--scope-files` が必須だったときに出ていた `EmptyScopeFiles` guard のエラーが発生しない [adr: knowledge/adr/2026-06-01-2300-review-fixer-self-resolve-scope-files.md#D1] [tasks: T007]
- [ ] [AC-10] review-fix-lead skill が `bin/sotp review files --scope <scope>` を実行してファイル一覧を取得する。orchestrator が `--scope-files` でファイルパスを fixer 起動コマンドに埋め込む指示が skill / `/track:review` から削除されている [adr: knowledge/adr/2026-06-01-2300-review-fixer-self-resolve-scope-files.md#D1, knowledge/adr/2026-06-01-2300-review-fixer-self-resolve-scope-files.md#D2] [tasks: T007, T008]
- [ ] [AC-11] `sotp review fix-local` を `--reviewer-model` なしで呼び出せる。fixer が入れ子で起動する reviewer コマンドは `--model` 指定なしで動作し、`agent-profiles.json` から round-type 別に model を自己解決する [adr: knowledge/adr/2026-06-01-2300-review-fixer-self-resolve-scope-files.md#D3] [tasks: T007]
- [ ] [AC-12] fixer 起動コマンドの引数が `--scope` / `--briefing-file` / `--track-id` / `--round-type` の4つに収まる（fixer 自身の `--model` は既存の省略可能フラグで変更なし）。`.github` 等のパスを含むファイルリストが Bash コマンド文字列に埋め込まれない [adr: knowledge/adr/2026-06-01-2300-review-fixer-self-resolve-scope-files.md#D1, knowledge/adr/2026-06-01-2300-review-fixer-self-resolve-scope-files.md#D3] [tasks: T007, T008]

## Related Conventions (Required Reading)
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- .claude/rules/04-coding-principles.md#No Panics in Library Code

## Signal Summary

### Stage 1: Spec Signals
🔵 40  🟡 0  🔴 0

