<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.1"
signals: { blue: 28, yellow: 0, red: 0 }
---

# CI 所要時間の短縮(キャッシュ戦略)+ review/commit ゲートのカタログ未生成耐性 + --lenient / --force 実行経路の撤去

## Goal

- [GO-01] 重量級ネイティブ依存追加後に約 14〜16 分に伸びた CI の所要時間を、ソースコード（機能実装・依存構成）を変更せず、キャッシュ戦略の見直しのみで短縮する。原因の特定と具体的な手段の選定は track 内の試行錯誤で行い、本 spec は結果・制約レベルでの要件のみ記述する [adr: knowledge/adr/2026-06-01-0336-ci-shorten-cache-strategy-only.md#D1]
- [GO-02] `track-active-gate` のシグナル再生成ステップ（`type-signals` 等）が、評価対象の層カタログが存在しない場合に no-op + warning で skip し非ゼロ終了しないようにする。これにより `track-local-review` と `track-commit-message` が Phase 0 / 1 の段階（型カタログ未生成）でも成功し、init 直後の ADR baseline commit フローが通る [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1]
- [GO-03] `type-signals` の `--lenient` フラグ（gate-vs-direct の区別）と `baseline-capture` の `--force` フラグ（baseline 上書き）を撤去し、両コマンドの実行経路を単純で安全な一本道にする。`type-signals` は views sync / catalogue-spec-signals と同様に全呼び出し経路でカタログ不在を無条件 skip し、`baseline-capture` は常に冪等（既存 baseline は保持）とする [adr: knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md#D1, knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md#D2]

## Scope

### In Scope
- [IN-01] CI キャッシュ設定ファイル（`.github/workflows/` 等のワークフロー定義、`Makefile.toml` のキャッシュ関連設定、Docker layer キャッシュ設定など）のうちキャッシュ戦略に関わる部分を調整する対象とする [adr: knowledge/adr/2026-06-01-0336-ci-shorten-cache-strategy-only.md#D1] [tasks: T002]
- [IN-02] `type-signals`（および同じく未生成の上流成果物を読む他のシグナルステップ）を、対象層のカタログファイルが存在しない場合に no-op + warning で skip し非ゼロ終了しないよう修正する対象とする。この skip 挙動は `views sync` / `catalogue-spec-signals` の既存の無条件 absent-skip に揃え、gate 経由呼び出しと直接呼び出しを区別しない（`--lenient` フラグ等の呼び出し経路依存分岐は撤去する） [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1, knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md#D1] [tasks: T003, T005]
- [IN-03] カタログが存在する層については従来どおりシグナルを評価し、🔴 は引き続きゲートを block する。skip は「カタログファイルが存在しない層」に限定し、カタログがある層の評価を省く fail-open は作らない [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1] [tasks: T003]
- [IN-04] `baseline-capture` の `--force` 実行経路（既存 baseline の上書き）を撤去し、baseline 取得を常に冪等な操作にする対象とする。既存 baseline が存在する場合は取得をスキップし、再取得が必要な場合は「baseline ファイルを削除してから capture を再実行する」運用に倒す [adr: knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md#D2] [tasks: T006]

### Out of Scope
- [OS-01] CI キャッシュ短縮作業におけるソースコード（機能実装・依存構成）の変更: ADR D1 が「ソースコード変更なし、キャッシュ戦略の見直しのみで対応する」と明示している。この制約は ADR 2026-06-01-0336 / T002 のキャッシュ戦略変更に限定し、ADR 2026-06-01-0406 / T003 の review gate 実装修正および ADR 2026-06-01-1206 の --lenient / --force 撤去を禁じない [adr: knowledge/adr/2026-06-01-0336-ci-shorten-cache-strategy-only.md#D1]
- [OS-02] CI 所要時間が伸びた具体的な原因の確定: ADR が「所要時間が伸びた具体的な原因は未確定であり、調査・試行錯誤の中で変わりうる」と明示している。spec は原因を断定しない [adr: knowledge/adr/2026-06-01-0336-ci-shorten-cache-strategy-only.md#D1]
- [OS-03] 使用するキャッシュ機構の事前確定（例: sccache layer 設定変更・GitHub Actions cache アクション切り替えなど）: 具体的な手段の選定は track での試行錯誤に委ねる。ADR は「具体的な手段の選定は track での試行錯誤を通じて行い、本 ADR では確定させない」と明示している [adr: knowledge/adr/2026-06-01-0336-ci-shorten-cache-strategy-only.md#D1]
- [OS-04] `track-local-review` / `track-commit-message` から `track-active-gate` 依存を丸ごと外す: カタログが存在する後続フェーズでもシグナル再生成が走らなくなり、reviewer / commit が古いシグナル状態を見る(hash mismatch / fail-open)。ADR Rejected Alternative A として記録されている [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1]
- [OS-05] Phase 0 専用の別レビュー経路の新設: `views sync` と同じ「入力不在なら skip」という一様なルールで足りるため、フェーズ別の経路を増やすのは不要な複雑化。ADR Rejected Alternative B として記録されている [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1]
- [OS-06] type-signals / baseline-capture の型契約変更（`MissingCataloguePolicy` enum 削除・`TypeSignalsExecutorPort::evaluate_layer` の policy 引数削除・`TypeSignalsRequest.lenient` 削除、および `RustdocBaselineCapturePort::capture` の force 引数削除・`BaselineCaptureRequest.force` 削除・`force_capture_rustdoc_baseline_for_layer` 削除等）のメソッドシグネチャ・具体的な型の選択: これらは Phase 2（type-design）が catalogue で宣言する契約レベルの事項であり、本 spec は振る舞い契約のみを記述する [adr: knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md#D1, knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md#D2]
- [OS-07] `catalogue-spec-signals` 側への `--lenient` フラグ追加（対称化）: 余分な機構をさらに増やす方向であり、ADR Rejected Alternative A として記録されている。本トラックは両コマンドの挙動を「フラグなし・無条件 absent-skip」で統一する方向をとる [adr: knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md#D1]
- [OS-08] `baseline-capture --force` の運用注意による存続: 危険な上書き経路をドキュメントだけで防ぐ案は ADR Rejected Alternative C として却下されている。本トラックは型・CLI レベルで経路自体を削除する [adr: knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md#D2]

## Constraints
- [CN-01] CI 設定の変更はキャッシュ戦略に限定する。ソースコード（`libs/` / `apps/` 配下の Rust ソース）・`Cargo.toml` / `Cargo.lock`（依存構成）・機能フラグ・スクリプトのロジックには手を加えない [adr: knowledge/adr/2026-06-01-0336-ci-shorten-cache-strategy-only.md#D1] [tasks: T002]
- [CN-02] シグナルステップの skip は「入力カタログファイルが存在しない層」に限定する。カタログが存在する層はすべて従来どおり評価し、🔴 は引き続きゲートを block する（fail-open を作らない） [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1] [tasks: T003]
- [CN-03] skip 時の挙動は `views sync` / `catalogue-spec-signals` の既存実装（カタログ不在を warning で skip・ハードエラーなし）と一貫させる。gate 経由・直接呼び出しを問わず、`type-signals`（および同種のシグナルステップ）は欠損カタログに対して同じ absent-skip の挙動を持ち、呼び出し経路によって振る舞いが変わらない（`--lenient` フラグによる gate-vs-direct 区別は存在しない） [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1, knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md#D1] [tasks: T003, T005]
- [CN-04] `cargo make ci`（fmt-check + clippy + nextest + deny + check-layers + verify-*）が pass する状態を各コミット時に維持する [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1] [tasks: T002, T003, T004, T005, T006]
- [CN-05] `baseline-capture` は常に冪等とする。既存 baseline ファイルが存在する場合は取得をスキップし上書きしない。baseline の再取得が必要な場合は「baseline ファイルを削除してから capture を再実行する」2 手順の運用とする。`--force` による 1 コマンド上書きは提供しない [adr: knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md#D2] [tasks: T006]

## Acceptance Criteria
- [ ] [AC-01] `track-local-review` と `track-commit-message` が、型カタログ（例: `domain-types.json`）が存在しない Phase 0 / 1 の状態でゼロ終了する。`type-signals evaluation failed for layer 'domain': failed to read catalogue ...` のようなハードエラーが出ない [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1] [tasks: T003]
- [ ] [AC-02] カタログが存在しない層を skip するとき、warning メッセージが標準エラーまたは標準出力に出力される（silent no-op でなく、skip した旨が観測できる） [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1] [tasks: T003]
- [ ] [AC-03] 型カタログが存在する層（Phase 2 以降）では `type-signals` がシグナルを従来どおり評価し、🔴 の場合にゲートが非ゼロ終了する（skip による fail-open が発生していない） [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1] [tasks: T003]
- [ ] [AC-04] キャッシュ戦略の調整後、CI の所要時間が従来（重量級ネイティブ依存追加前）の水準に近づく、または明確に短縮される。ソースコード（`libs/` / `apps/` の Rust ソース・`Cargo.toml` / `Cargo.lock`）に変更がない [adr: knowledge/adr/2026-06-01-0336-ci-shorten-cache-strategy-only.md#D1] [tasks: T001, T002]
- [ ] [AC-05] `cargo make ci` が pass する（fmt-check + clippy + nextest + deny + check-layers + verify-* の全ステップ） [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1] [tasks: T004]
- [ ] [AC-06] `type-signals` をユーザーが直接（gate 経由でなく）呼び出したとき、カタログ不在の層は gate 経由と同じ absent-skip の挙動をとり、エラーにならない（`--lenient` フラグなしの状態で gate/direct の区別が消えている） [adr: knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md#D1] [tasks: T005]
- [ ] [AC-07] `baseline-capture` を既存 baseline が存在する状態で実行しても、既存 baseline が上書きされない（冪等に skip される） [adr: knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md#D2] [tasks: T006]
- [ ] [AC-08] `baseline-capture --source-workspace` が引き続き利用可能であり、main の git worktree から baseline を取得する正規の運用が維持される [adr: knowledge/adr/2026-06-01-1206-remove-lenient-and-force-flag-paths.md#D2] [tasks: T006]

## Related Conventions (Required Reading)
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- .claude/rules/04-coding-principles.md#No Panics in Library Code

## Signal Summary

### Stage 1: Spec Signals
🔵 28  🟡 0  🔴 0

