<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 38, yellow: 0, red: 0 }
---

# composition root を専用 crate (apps/cli-composition) に切り出す

## Goal

- [GO-01] composition root の責務を `apps/cli-composition`（crate 名 `cli_composition`）に集約し、`architecture-rules.json` に新層エントリを追加することで、cli → infrastructure 直接依存をコンパイル時に遮断できるアーキテクチャ境界を確立する [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D1]
- [GO-02] `CliApp` facade と `CommandOutcome` 統一戻り値を公開 API とすることで、bin が parse + emit のみの極薄な entry point になり、DI 配線・ユースケース呼び出し・出力整形がすべて `cli_composition` 内に閉じる [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D2]
- [GO-03] `libs/infrastructure/src/review_v2/cli_composition.rs` をはじめとする infrastructure 層に押し出された composition logic を正しい層（`apps/cli-composition`）へ移動し、infrastructure 層の adapter 型のみ公開する非対称を解消する [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D4]

## Scope

### In Scope
- [IN-01] `apps/cli-composition` ディレクトリを新設し、`architecture-rules.json` に `cli_composition` 層エントリ（`may_depend_on: ["domain", "infrastructure", "usecase"]`、`tddd: { "enabled": false }`）を追加する [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D1] [tasks: T001]
- [IN-02] `Cargo.toml`（workspace ルート）の `members` に `"apps/cli-composition"` を追加し、`apps/cli-composition/Cargo.toml` を新設して `domain` / `usecase` / `infrastructure` 依存を宣言する [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D6] [tasks: T001]
- [IN-03] `CliApp` facade struct と `CommandOutcome` 統一戻り値型を `apps/cli-composition` の公開 API として実装する。facade の公開面には string / path / primitive / composition 自身が定義する DTO のみを出し、generic interactor は composition 内部に private に閉じる [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D2] [tasks: T002, T004, T005]
- [IN-04] `apps/cli` の `may_depend_on` を `["cli_composition"]` に変更し、`apps/cli/Cargo.toml` から `usecase` と `infrastructure` の依存を削除して `cli_composition` を追加する [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D3] [tasks: T006]
- [IN-05] `libs/infrastructure/src/review_v2/cli_composition.rs`（1507 行）を `apps/cli-composition/src/review_v2/` へ移動し、コンテキスト境界に従ってサブモジュールに分割する（各ファイル 700 行以内） [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D4] [tasks: T003, T004, T005]
- [IN-06] 移動後、`libs/infrastructure/src/review_v2/mod.rs` は adapter 型（`CodexReviewer`, `ClaudeReviewer`, `GitDiffGetter`, `SystemReviewHasher`, `FsReviewStore`, `FsCommitHashStore`, `load_v2_scope_config`）の公開のみ残し、composition function の re-export を停止する [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D4] [tasks: T003, T005, T006]
- [IN-07] 移行を re-export shim で CI 緑を維持しながらコマンドファミリー単位で段階コミットし、全コマンドの移動が完了した最終コミットで `architecture-rules.json` / `deny.toml` / `Cargo.toml` を一括更新して `cli.may_depend_on = ["cli_composition"]` を固定する [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D5] [tasks: T003, T006]
- [IN-08] `deny.toml` を手書き更新し、`infrastructure` の `wrappers` を `["cli"]` から `["cli_composition"]` に変更し、新規 `cli_composition` の `wrappers=["cli"]` を追加し、`usecase` の `wrappers` を更新する [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D6] [tasks: T006]

### Out of Scope
- [OS-01] `apps/cli-composition` 内部のモジュール分割案・trait 設計・型の具体的な定義: これらは Phase 2（type-design）が扱う [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D2]
- [OS-02] `apps/cli` 内モジュール分割のみ（新 crate 作成なし、Rejected Alternative A）: deny.toml とコンパイル時強制は crate 粒度で機能するため、module 分割では cli → infra 依存を実際に遮断できない [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D1]
- [OS-03] `libs/composition` への配置（Rejected Alternative B）: composition root は entry point ごとの最外殻であり再利用ライブラリではないため `libs/` ではなく `apps/` に置く [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D1]
- [OS-04] bin の usecase 依存の保持（DTO 取得目的、Rejected Alternative E）: `CommandOutcome` 統一戻り値で bin は usecase 型を一切必要としない [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D3]
- [OS-05] 全ファイルを一度に動かす single big-bang コミット（Rejected Alternative F）: 巨大 diff で review が困難になり、移行中に CI 緑を保てない [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D5]
- [OS-06] web / gRPC など 2 つ目の entry point のための composition root 追加: 本 track は CLI 専用の `apps/cli-composition` に限定し、multi entry point 対応は将来 track とする [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D1]
- [OS-07] `architecture-rules.json` の自動生成による `deny.toml` 更新: D6 で「手書き更新」と明示されており、自動生成は本 track の対象外 [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D6]
- [OS-08] `hexagonal-architecture.md` convention の「CLI as Composition Root」節の更新: ADR Consequences に「本 ADR の実装後にこの節を更新する」と記されているが、convention 更新は本 track の実装完了後の後続作業であり spec の acceptance_criteria に含めない [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D1]

## Constraints
- [CN-01] `apps/cli` の `may_depend_on` 最終状態は `["cli_composition"]` のみ。中間状態（`cli_composition` + `infrastructure` + `usecase` を含む移行途中の状態）は main にマージしない [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D3, knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D5] [tasks: T006]
- [CN-02] `apps/cli-composition` の公開面（`CliApp` facade のメソッド引数・戻り値）には string / path / primitive / composition 自身が定義する DTO のみを出す。`usecase` / `domain` / `infrastructure` の型を facade 公開面に漏らさない [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D2] [tasks: T002, T004]
- [CN-03] 移動後のファイルはそれぞれ 700 行以内に収める。`libs/infrastructure/src/review_v2/cli_composition.rs`（1507 行）は移動時にサブモジュールに分割することで制限を満たす [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D4] [tasks: T003]
- [CN-04] enforcement は `architecture-rules.json`（SSoT）/ `deny.toml`（手書き）/ `Cargo.toml` の更新で成立させる。新規の lint 機構・追加 verify サブコマンド・Rust `#[allow(...)]` 属性や CI バイパスフラグのような ad-hoc な移行期 allow list は導入しない。D5 の段階コミット戦略に従い `architecture-rules.json` / `deny.toml` を段階的に更新して中間状態を通過することは許容される（最終的に D3/D6 が定める状態に収束させる） [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D6] [tasks: T006]
- [CN-05] 移行中は re-export shim を使い CI が常に緑の状態を維持する。コンパイルエラーや CI 失敗が発生する中間コミットは track ブランチ上にも残さない [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D5] [tasks: T001, T002, T003, T004, T005, T006]

## Acceptance Criteria
- [ ] [AC-01] `apps/cli-composition` ディレクトリが存在し、`architecture-rules.json` に `cli_composition` 層エントリ（`may_depend_on: ["domain", "infrastructure", "usecase"]`、`tddd.enabled: false`）が追加されている [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D1] [tasks: T001]
- [ ] [AC-02] `apps/cli-composition/Cargo.toml` が存在し、workspace ルートの `Cargo.toml` の `members` に `"apps/cli-composition"` が含まれる [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D6] [tasks: T001]
- [ ] [AC-03] `apps/cli-composition/src/` に `CliApp` struct と `CommandOutcome` 型が公開 API として存在し、`CliApp` のメソッド引数・戻り値に `usecase` / `domain` / `infrastructure` の型が出現しない [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D2] [tasks: T002, T004]
- [ ] [AC-04] `apps/cli/Cargo.toml` の `[dependencies]` にワークスペース内レイヤー依存として `usecase` と `infrastructure` が存在せず、`cli_composition` のみがワークスペースレイヤー依存として宣言されている（`clap` / `anyhow` 等の外部クレート依存は本 AC の対象外） [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D3] [tasks: T006]
- [ ] [AC-05] `architecture-rules.json` の `apps/cli` 層エントリの `may_depend_on` が `["cli_composition"]` のみである [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D3] [tasks: T006]
- [ ] [AC-06] `apps/cli/src/` 配下のすべての Rust ファイルに `use infrastructure::` / `use usecase::` の直接 import が存在しない（bin は `cli_composition` の公開 API のみを使う） [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D3] [tasks: T004, T005, T006]
- [ ] [AC-07] `libs/infrastructure/src/review_v2/` に composition function が存在しない。`libs/infrastructure/src/review_v2/mod.rs` は adapter 型（`CodexReviewer`, `ClaudeReviewer`, `GitDiffGetter`, `SystemReviewHasher`, `FsReviewStore`, `FsCommitHashStore`, `load_v2_scope_config`）の公開のみを含む [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D4] [tasks: T003, T005, T006]
- [ ] [AC-08] `apps/cli-composition/src/` 配下のすべての Rust ファイルが 700 行以内である [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D4] [tasks: T003]
- [ ] [AC-09] `deny.toml` が更新されており、`infrastructure` の `wrappers` に `cli` が含まれず `cli_composition` が含まれる。新規 `cli_composition` の `wrappers=["cli"]` エントリが存在する [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D6] [tasks: T006]
- [ ] [AC-10] `cargo make deny` が pass する（`deny.toml` の最終状態で cli が infrastructure / usecase を直接依存しておらず、禁止違反がない状態を確認する） [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D6] [tasks: T006]
- [ ] [AC-11] `bin/sotp verify layers` が pass する（`architecture-rules.json` の最終状態で cli.may_depend_on = ["cli_composition"] のみとなっており、cli → cli_composition 依存のみが許可された状態を確認する） [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D6] [tasks: T006]
- [ ] [AC-12] `architecture-rules-verify-sync`（`scripts/architecture_rules.py verify-sync`）が pass する（`architecture-rules.json` から期待値を計算し、手書き `deny.toml` と `Cargo.toml` workspace members との一致を検証） [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D6] [tasks: T006]
- [ ] [AC-13] `cargo make ci` の全項目（fmt-check + clippy + nextest + deny + check-layers + verify-*）が pass する [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D5, knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D6] [tasks: T001, T002, T003, T004, T005, T006]
- [ ] [AC-14] 既存 CLI コマンドの外部 CLI 振る舞い（引数・出力フォーマット・exit code）が変化しない。既存の統合テストおよび acceptance test が pass する [adr: knowledge/adr/2026-05-27-0110-composition-root-dedicated-crate.md#D5] [tasks: T004, T005, T006]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/hexagonal-architecture.md#CLI as Composition Root
- knowledge/conventions/hexagonal-architecture.md#Adapter Rules
- .claude/rules/04-coding-principles.md#Trait-Based Abstraction (Hexagonal Architecture)
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/04-coding-principles.md#Module Size
- .claude/rules/05-testing.md#Test Structure

## Signal Summary

### Stage 1: Spec Signals
🔵 38  🟡 0  🔴 0

