<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 20, yellow: 0, red: 0 }
---

# CI 所要時間の短縮(キャッシュ戦略)+ review/commit ゲートのカタログ未生成耐性

## Goal

- [GO-01] 重量級ネイティブ依存追加後に約 14〜16 分に伸びた CI の所要時間を、ソースコード（機能実装・依存構成）を変更せず、キャッシュ戦略の見直しのみで短縮する。原因の特定と具体的な手段の選定は track 内の試行錯誤で行い、本 spec は結果・制約レベルでの要件のみ記述する [adr: knowledge/adr/2026-06-01-0336-ci-shorten-cache-strategy-only.md#D1]
- [GO-02] `track-active-gate` のシグナル再生成ステップ（`type-signals` 等）が、評価対象の層カタログが存在しない場合に no-op + warning で skip し非ゼロ終了しないようにする。これにより `track-local-review` と `track-commit-message` が Phase 0 / 1 の段階（型カタログ未生成）でも成功し、init 直後の ADR baseline commit フローが通る [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1]

## Scope

### In Scope
- [IN-01] CI キャッシュ設定ファイル（`.github/workflows/` 等のワークフロー定義、`Makefile.toml` のキャッシュ関連設定、Docker layer キャッシュ設定など）のうちキャッシュ戦略に関わる部分を調整する対象とする [adr: knowledge/adr/2026-06-01-0336-ci-shorten-cache-strategy-only.md#D1] [tasks: T002]
- [IN-02] `track-active-gate` から起動される `type-signals` ステップ（および同じく未生成の上流成果物を読む他のゲート内シグナルステップ）を、対象層のカタログファイルが存在しない場合に no-op + warning で skip し、非ゼロ終了しないよう修正する対象とする。ユーザーが直接実行する `sotp track type-signals` の strict な挙動は変更対象にしない [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1] [tasks: T003]
- [IN-03] カタログが存在する層については従来どおりシグナルを評価し、🔴 は引き続きゲートを block する。skip は「カタログファイルが存在しない層」に限定し、カタログがある層の評価を省く fail-open は作らない [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1] [tasks: T003]

### Out of Scope
- [OS-01] CI キャッシュ短縮作業におけるソースコード（機能実装・依存構成）の変更: ADR D1 が「ソースコード変更なし、キャッシュ戦略の見直しのみで対応する」と明示している。この制約は ADR 2026-06-01-0336 / T002 のキャッシュ戦略変更に限定し、ADR 2026-06-01-0406 / T003 の review gate 実装修正を禁じない [adr: knowledge/adr/2026-06-01-0336-ci-shorten-cache-strategy-only.md#D1]
- [OS-02] CI 所要時間が伸びた具体的な原因の確定: ADR が「所要時間が伸びた具体的な原因は未確定であり、調査・試行錯誤の中で変わりうる」と明示している。spec は原因を断定しない [adr: knowledge/adr/2026-06-01-0336-ci-shorten-cache-strategy-only.md#D1]
- [OS-03] 使用するキャッシュ機構の事前確定（例: sccache layer 設定変更・GitHub Actions cache アクション切り替えなど）: 具体的な手段の選定は track での試行錯誤に委ねる。ADR は「具体的な手段の選定は track での試行錯誤を通じて行い、本 ADR では確定させない」と明示している [adr: knowledge/adr/2026-06-01-0336-ci-shorten-cache-strategy-only.md#D1]
- [OS-04] `track-local-review` / `track-commit-message` から `track-active-gate` 依存を丸ごと外す: カタログが存在する後続フェーズでもシグナル再生成が走らなくなり、reviewer / commit が古いシグナル状態を見る(hash mismatch / fail-open)。ADR Rejected Alternative A として記録されている [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1]
- [OS-05] Phase 0 専用の別レビュー経路の新設: `views sync` と同じ「入力不在なら skip」という一様なルールで足りるため、フェーズ別の経路を増やすのは不要な複雑化。ADR Rejected Alternative B として記録されている [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1]
- [OS-06] type-signals 評価ロジックの型設計・メソッドシグネチャ・種別選択（newtype / enum / typestate 等）: これらは Phase 2（type-design）が確定する契約レベルの事項であり、本 spec は振る舞い契約のみを記述する [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1]

## Constraints
- [CN-01] CI 設定の変更はキャッシュ戦略に限定する。ソースコード（`libs/` / `apps/` 配下の Rust ソース）・`Cargo.toml` / `Cargo.lock`（依存構成）・機能フラグ・スクリプトのロジックには手を加えない [adr: knowledge/adr/2026-06-01-0336-ci-shorten-cache-strategy-only.md#D1] [tasks: T002]
- [CN-02] シグナルステップの skip は「入力カタログファイルが存在しない層」に限定する。カタログが存在する層はすべて従来どおり評価し、🔴 は引き続きゲートを block する（fail-open を作らない） [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1] [tasks: T003]
- [CN-03] skip 時の挙動は `views sync` の既存実装（カタログ不在を warning で skip・ハードエラーなし）と一貫させる。ゲート内の異なるステップが欠損入力に対して非対称な挙動を持たないようにする [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1] [tasks: T003]
- [CN-04] `cargo make ci`（fmt-check + clippy + nextest + deny + check-layers + verify-*）が pass する状態を各コミット時に維持する [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1] [tasks: T004]

## Acceptance Criteria
- [ ] [AC-01] `track-local-review` と `track-commit-message` が、型カタログ（例: `domain-types.json`）が存在しない Phase 0 / 1 の状態でゼロ終了する。`type-signals evaluation failed for layer 'domain': failed to read catalogue ...` のようなハードエラーが出ない [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1] [tasks: T003]
- [ ] [AC-02] カタログが存在しない層を skip するとき、warning メッセージが標準エラーまたは標準出力に出力される（silent no-op でなく、skip した旨が観測できる） [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1] [tasks: T003]
- [ ] [AC-03] 型カタログが存在する層（Phase 2 以降）では `type-signals` がシグナルを従来どおり評価し、🔴 の場合にゲートが非ゼロ終了する（skip による fail-open が発生していない） [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1] [tasks: T003]
- [ ] [AC-04] キャッシュ戦略の調整後、CI の所要時間が従来（重量級ネイティブ依存追加前）の水準に近づく、または明確に短縮される。ソースコード（`libs/` / `apps/` の Rust ソース・`Cargo.toml` / `Cargo.lock`）に変更がない [adr: knowledge/adr/2026-06-01-0336-ci-shorten-cache-strategy-only.md#D1] [tasks: T001, T002]
- [ ] [AC-05] `cargo make ci` が pass する（fmt-check + clippy + nextest + deny + check-layers + verify-* の全ステップ） [adr: knowledge/adr/2026-06-01-0406-review-gate-tolerate-missing-catalogue.md#D1] [tasks: T004]

## Related Conventions (Required Reading)
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- .claude/rules/04-coding-principles.md#No Panics in Library Code

## Signal Summary

### Stage 1: Spec Signals
🔵 20  🟡 0  🔴 0

