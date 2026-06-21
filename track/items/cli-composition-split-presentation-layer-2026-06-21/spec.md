<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 42, yellow: 0, red: 0 }
---

# CLI delivery 側の責務分離 — composition root と primary adapter への分解

## Goal

- [GO-01] CLI delivery 側に `cli_driver`（primary adapter / controller）を新設し、bin / composition root / primary adapter の 3 層に分解することで、DI を行う wire 責務（`cli_composition`）と use case を呼んで結果を整形する invoke + render 責務（`cli_driver`）を層レベルで分離する。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D1, knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D3]
- [GO-02] `cli_composition` を純 DI composition root（wire のみ、invoke しない）にし、god-facade `CliApp`（51 メソッド / 27 ファイル）を bounded-context 別 `CompositionRoot` 構造に分解することで、composition root の canonical な役割を回復する。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D2]
- [GO-03] `cli_composition` 内の多ステップオーケストレーション（signal chain sequencing、diff-fragment pipeline、PR review polling など）を usecase の application service へ移譲することで、composition root をアダプタ配線に特化させる。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D4]
- [GO-04] `cli_composition` 内の adapter-outside-infrastructure 7 件を解消する。6 件の port 実装 adapter は `libs/infrastructure` へ移設し、`LazyBranchReader` は除去して infrastructure の `SystemGitRepo` 実装に一本化することで、port 実装と DI 配線の定義場所を正規化する。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D7]
- [GO-05] `apps/cli/src/main.rs` の `emit_archived_track_subcommand` が行っている直接 I/O（`std::fs` / `serde_json` / `chrono::Utc::now()`）を infrastructure adapter 化し、bin を parse + dispatch + emit のみの thin-bin に戻す。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D8]

## Scope

### In Scope
- [IN-01] `apps/cli-driver`（crate 名 `cli_driver`）の新設。driving adapter（primary adapter / controller）として use case を注入保持し、`handle(input)` で invoke + render して `CommandOutcome` を返す責務を担う。`CommandOutcome` 型と render 関数群をこの層に置く。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D1] [tasks: T014, T021]
- [IN-02] `cli_driver` 層が `usecase` のみに依存すること（`cli_driver → usecase`）。DI はしない（注入される側）。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D1] [tasks: T014, T021]
- [IN-03] `cli_composition` の責務を DI（object graph の組み立て）のみに絞る。secondary adapter → interactor → driving adapter を構築し、use case を driving adapter に注入して返す。use case を invoke しない。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D2] [tasks: T010, T011, T012, T013, T021]
- [IN-04] typed `CompositionError` の新設。`cli_composition` 公開メソッドの stringly-typed `Result<CommandOutcome, String>` を `Result<_, CompositionError>` に置き換える。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D2] [tasks: T010]
- [IN-05] god-facade `CliApp`（stateless unit struct / 51 メソッド / 27 ファイル）の廃止と、bounded-context 別 `CompositionRoot` 構造への分解。`CliApp` という単一型は残さない。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D2] [tasks: T010, T011, T012, T013]
- [IN-06] invoke と render を同一層（`cli_driver`）に置く。render は cli_driver 内 module であり別 crate にしない。bin は driver を受け取り `driver.handle(input)` を呼んで `CommandOutcome` を emit するのみ。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D3] [tasks: T014, T015, T016, T017, T018, T019, T021]
- [IN-07] multi-step オーケストレーション・統合ロジック（signal chain sequencing、diff-fragment pipeline、PR polling loop など）の usecase application service への移譲。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D4] [tasks: T005, T006, T007, T008, T021]
- [IN-08] usecase 出力 DTO を invoke + render の境界として流用する（`2026-04-30-0848` の DTO を継続利用）。bin は `usecase` / `domain` を import しない（thin-bin 維持）。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D5] [tasks: T020, T021]
- [IN-09] 移行を専用 1 track 内の段階 commit で進める。re-export shim で CI 緑を保ちつつ command-family 単位で移動し、最終 commit で `architecture-rules.json` / `deny.toml` / `Cargo.toml` の依存グラフを一括 flip する。中間状態は main にマージしない。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D6] [tasks: T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012, T013, T014, T015, T016, T017, T018, T019, T020, T021]
- [IN-10] reviewer severity policy（`.harness/custom/review-prompts/`）の整備: `cli_driver.md` 新設、`cli.md` と `cli_composition.md` の更新を実装 track 内で実施する。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D6] [tasks: T022]
- [IN-11] `cli_composition` 内の adapter-outside-infrastructure 7 件を解消する。FsReviewGateStateAdapter / FsRefVerifyGateStateAdapter / RecordingDryAgent / NullInsertIndexProxy / NoopSemanticIndexPort / NoOpDryApprovalService の 6 件は `libs/infrastructure` へ移設する。`LazyBranchReader` は UFCS 回避用の二重 adapter であり、呼び出し側の trait 曖昧性解消で除去して infrastructure の `SystemGitRepo` 実装に一本化する。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D7] [tasks: T002, T003, T004]
- [IN-12] `apps/cli/src/main.rs` の `emit_archived_track_subcommand`（`std::fs` / `serde_json` / `chrono::Utc::now()`）を infrastructure adapter（時刻取得・fs 書き込み）として切り出し、composition が wire して driver 経由で呼ぶ経路にする。bin は parse + dispatch + emit のみに戻す。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D8] [tasks: T001, T009, T021]

### Out of Scope
- [OS-01] 以下の項目は本 track（1328）のスコープ外であり、後続 track（1420）で対応する。

【スコープ外: 1420 D1】`architecture-rules.json` の `cli` / `cli_composition` / `cli_driver` レイヤーの `tddd.enabled: true` フリップ（TDDD 有効化）、および各層の `catalogue_spec_signal.enabled: true` / `schema_export.method: rustdoc` ブロックの設定。注意: `cli_driver` 層エントリ自体の **追加**（`apps/cli-driver` を `layers` に登録し `may_depend_on: ["usecase"]` を設定する行）は 1328 D1 の最終 flip commit で行われ、本 track に属する。スコープ外は `tddd.enabled` フリップのみ。

【スコープ外: 1420 D2】`DataRole` enumeration への `CompositionRoot` / `PrimaryAdapter` 新 variant の追加（codec / signal evaluator / renderer / 全 `role ==` 比較の更新を含む）。

【スコープ外: 1420 D3】`.harness/catalogue-lint/` の config / preset エントリ追加（cli 系 3 層の per-layer `KindLayerConstraint` ルール群、および `PrimaryAdapter` の `NoRoleInMethodSignature` ルール）。

【スコープ外: 1420 派生作業】`cli-types.json` / `cli_composition-types.json` / `cli_driver-types.json` の型カタログ起草、および `knowledge/conventions/type-designer-kind-selection.md` R1 マトリクスへの `cli` / `cli_composition` / `cli_driver` 3 列と `CompositionRoot` / `PrimaryAdapter` 2 行の追加。

【本 track（1328）に属し除外されない項目】以下は 1328 D1/D6/D6(h)/D7/D8 に基づく本 track のスコープ内であり、上記除外の対象ではない: (a) `architecture-rules.json` の `layers` への `cli_driver` 層エントリ追加・`cli_composition.may_depend_on` / `cli.may_depend_on` の更新（D1 最終 flip）、(b) `deny.toml` / `Cargo.toml` workspace members の対応更新（D1 最終 flip）、(c) `.harness/custom/review-prompts/cli_driver.md` 新設・`cli.md` / `cli_composition.md` 更新（D6(h)）、(d) adapter-outside-infrastructure 7 件の解消（6 件の infrastructure 移設 + `LazyBranchReader` 除去）（D7）、(e) cli bin telemetry 直接 I/O の infrastructure adapter 化（D8）。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D6] [tasks: T021]
- [OS-02] render を独立 presentation 層（`cli-presentation` crate）に分離する 4 層案。invoke と render は primary adapter の双方向変換の表裏であり、現時点での over-decomposition として却下。複数 entry point や render 差し替えが生じた時点で再評価する。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D3] [tasks: T014, T015, T016, T017, T018, T019]
- [OS-03] `hexagonal-architecture.md` の `CLI as Composition Root` 節の更新（3 層構成への改訂）。本 track は要件のみ実装し、convention doc の文面更新は実装完了後の別作業とする。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D6] [tasks: T022]
- [OS-04] composition root が wired use case を返し bin が直接 invoke する案（Alternative D）。bin が usecase の port / Command / Result 型を知る必要があり thin-bin が崩れるため却下。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D5] [tasks: T020]

## Constraints
- [CN-01] `cli_driver` は `usecase` のみに依存する（`cli_driver → usecase`）。`domain` / `infrastructure` / `cli_composition` への直接依存は禁止。DI をしない（注入される側）。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D1] [tasks: T014, T015, T016, T017, T018, T019, T020, T021]
- [CN-02] `cli_composition` は use case を invoke してはならない（invoke は `cli_driver` の責務）。composition root はアダプタ構築と wire のみ担う。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D2] [tasks: T010, T011, T012, T013, T021]
- [CN-03] `cli_composition` の公開 boundary エラーは typed `CompositionError` でなければならない。`Result<CommandOutcome, String>` は禁止。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D2] [tasks: T010]
- [CN-04] bin（`apps/cli`）は `usecase` / `domain` を直接 import してはならない。bin が知るのは `cli_composition`（wiring）と `cli_driver`（driving adapter の型 / `CommandOutcome`）のみ。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D5] [tasks: T020, T021]
- [CN-05] port を実装する struct（secondary adapter）は `libs/infrastructure` に定義しなければならない。`cli_composition` は adapter を wire するが定義はしない。null-object stub は infrastructure companion stub にするか、infra 非依存なら usecase 提供の test-double にする。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D7] [tasks: T002, T003, T004]
- [CN-06] 移行中の各 commit は CI 緑を維持しなければならない（re-export shim を許容するが broken CI 状態での commit は禁止）。`architecture-rules.json` / `deny.toml` / `Cargo.toml` の依存グラフ更新は最終 commit で一括 flip する。`cli-driver` crate 実体の追加と architecture-rules 更新は同一 commit で行うこと（crate 実体なしに層エントリだけ追加すると `sotp verify layers` が CI を割るため）。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D6] [tasks: T021]
- [CN-07] multi-step オーケストレーション（signal chain sequencing / diff-fragment pipeline / PR polling など）は driving adapter（`cli_driver`）ではなく usecase の application service に置かなければならない。driving adapter は単一 use case の invoke + render に留める。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D4] [tasks: T005, T006, T007, T008]
- [CN-08] bin に直接 I/O（`std::fs` / `serde_json` / `chrono::Utc::now()` 相当のシステムコール）を持ってはならない。telemetry 永続化を含む全 I/O は infrastructure adapter + driver 経由にする。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D8] [tasks: T001, T009, T021]

## Acceptance Criteria
- [ ] [AC-01] `apps/cli-driver` crate（`cli_driver`）が存在し、`architecture-rules.json` に `cli_driver` 層エントリ（`may_depend_on: ["usecase"]`）が追加されており、`cargo make check-layers` が pass する。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D1] [tasks: T021]
- [ ] [AC-02] `cli_driver` の各 driving adapter は注入された use case を保持し（フィールドに持つ）、`handle(input)` で invoke + render して `CommandOutcome` を返す。`cli_driver` のソースに `domain::` / `infrastructure::` / `cli_composition::` への直接 `use` / `import` が存在しない。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D1, knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D3] [tasks: T014, T015, T016, T017, T018, T019, T021]
- [ ] [AC-03] `cli_composition` のソースに invoke 呼び出し（use case の application method call）が存在しない。`cli_composition` は adapter 構築と wire のみを行う。audit README に列挙された cli-composition-orchestration-leak カテゴリ（`pr/poll.rs:248-396` の polling loop など）が cli_composition から除去されている。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D2, knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D4] [tasks: T005, T006, T007, T008, T011, T012, T013, T021]
- [ ] [AC-04] god-facade `pub struct CliApp;`（unit struct）が `apps/cli-composition/src/lib.rs` に存在しない。bounded-context 別 `CompositionRoot` 構造体が存在し、各 context の driver を構築する wiring のみを担っている。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D2] [tasks: T013, T021]
- [ ] [AC-05] `CompositionError` 型が `cli_composition` に定義されており、`cli_composition` の公開メソッドが `Result<CommandOutcome, String>` を返す箇所が存在しない。audit findings の stringly-typed-error-boundary カテゴリ（51 メソッド全件）が解消されている。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D2] [tasks: T010]
- [ ] [AC-06] `cli_composition` のソースに `format!` / `writeln!` / `println!` / `eprintln!` / `serde_json::json!` を使ったユーザー向け文字列組み立てが存在しない。audit findings の presentation-in-composition-root（5 件）/ json-output-assembly-in-composition-root（4 件）/ pre-formatted-stdout-stderr-in-composer-methods（7 件）/ cli-composition-presentation-leak（1 件）の計 17 件が解消されている。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D1, knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D3] [tasks: T014, T015, T016, T017, T018, T019, T021]
- [ ] [AC-07] multi-step-orchestration カテゴリ（audit: 6 件）が cli_composition から除去されており、各オーケストレーション（signal chain sequencing / diff-fragment pipeline / dry_write メトリクス集計 / dry_check_approved fragment-ref derivation など）が usecase application service に移設されている。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D4] [tasks: T005, T006, T007, T008, T021]
- [ ] [AC-08] bin（`apps/cli`）のソースに `usecase::` / `domain::` への直接 `use` / `import` が存在しない。bin は `cli_composition` と `cli_driver` のみを参照する。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D5] [tasks: T020, T021]
- [ ] [AC-09] adapter-outside-infrastructure カテゴリ（audit: 7 件）が解消されている。FsReviewGateStateAdapter / FsRefVerifyGateStateAdapter / RecordingDryAgent / NullInsertIndexProxy / NoopSemanticIndexPort / NoOpDryApprovalService が `libs/infrastructure` に移設されており、LazyBranchReader が除去されている（UFCS 曖昧性が呼び出し側で解消されている）。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D7] [tasks: T002, T003, T004]
- [ ] [AC-10] `apps/cli/src/main.rs` に `std::fs` / `serde_json` / `chrono::Utc::now()` を使った直接 I/O が存在しない。cli-bin-business-logic カテゴリ（audit: `main.rs:247-294`）の 1 件が解消されており、telemetry 永続化が infrastructure adapter 経由の driver 呼び出しになっている。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D8] [tasks: T001, T009, T021]
- [ ] [AC-11] reviewer severity policy が新 3 層構成に追従している: `cli_driver.md` が新設され、`cli.md` と `cli_composition.md` が更新されており、各ファイルに ADR D2/D3/D5/D7/D8 の review 観点が記述されている。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D6] [tasks: T022]
- [ ] [AC-12] 最終 flip commit において `architecture-rules.json` の `layers` に `cli_driver` エントリが追加され、`cli_composition` の `may_depend_on` に `cli_driver` が追加され、`cli` の `may_depend_on` に `cli_driver` が追加されており、`cargo make deny` および `cargo make check-layers` が pass する。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D1, knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D6] [tasks: T021]
- [ ] [AC-13] `cargo make ci`（fmt-check + clippy + nextest + deny + check-layers + verify-*）が pass する。移行の各段階 commit でも CI が緑を維持している（broken 中間状態が main にマージされていない）。 [adr: knowledge/adr/2026-06-21-1328-cli-composition-split-presentation-layer.md#D6] [tasks: T001, T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012, T013, T014, T015, T016, T017, T018, T019, T020, T021, T022]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md#CLI as Composition Root
- knowledge/conventions/hexagonal-architecture.md#Adapter Rules
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/coding-principles.md#No Panics in Library Code
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/track-lifecycle.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 42  🟡 0  🔴 0

