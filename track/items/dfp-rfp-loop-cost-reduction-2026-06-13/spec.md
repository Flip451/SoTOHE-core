<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 42, yellow: 0, red: 0 }
---

# DFP⇄RFP 往復コストの削減

## Goal

- [GO-01] dfl の DFP ループを効率化し、violation が無い場合（no-fix run）は `cargo make ci-rust` と冗長な 2 度目の `dry write` を省略する。violation ゼロの定常ケースで DFP のコストを `dry write`（判定）コストに近づける（D1）。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D1]
- [GO-02] DFP⇄RFP の fixpoint 判定（3 ゲートすべてが同時に green になったか）を単一の決定的コマンドに機械化し、orchestrator が収束判定ロジックをプロンプト運用で抱える必要をなくす（D2）。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D2]
- [GO-03] `sotp dry write` の判定 fan-out を直列から上限付き並列に変え、未検証ペアが多い run の wall-clock を削減する（D3）。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D3]
- [GO-04] dry-checker の判定を較正付き 2 段構成（fast tier 軽量モデル + 既知違反プローブ較正 → 必要時のみ重量級 tier へ escalation）にし、判定 1 回あたりの単価を削減する（D4）。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D4]
- [GO-05] `sotp dry check-approved` を埋め込み・類似検索なしの純読み取りゲート（staleness FragmentRef 照合 + all-resolved 確認）にし、dfl ループの先頭 check・fixpoint 合成・commit ゲートの反復評価コストを下げる（D5）。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D5]

## Scope

### In Scope
- [IN-01] dfl の DFP ループ定義（`.claude/agents/dry-fix-lead.md`）を変更し、violation ゼロ時は step 1 の `sotp dry check-approved`（Approved）→ completed で終わり、`cargo make ci-rust` と 2 度目の `dry write` をスキップする分岐を追加する。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D1] [tasks: T006]
- [IN-02] DFP⇄RFP の収束状態（dry gate / review per-scope hash 失効 + verdict / ref-verify gate の 3 ゲート）を合成し、次に必要なフェーズ（DFP 再入 / RFP 対象 scope 列挙 / ref-verify 再実行 / commit 可）を出力する単一の決定的コマンド（`sotp track fixpoint-resolve` 相当）を新設する。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D2] [tasks: T014, T015]
- [IN-03] `sotp dry write` の処理を 2 相（照会相: 全 diff fragment の類似照会 → ペア識別子重複排除 → 未検証ペア確定、判定相: 確定ペアへの上限付き並列 fan-out）に分け、並列度上限を `.harness/config/dry-check.json` の `max_parallelism` フィールドで設定可能にする。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D3] [tasks: T008, T009, T010, T016]
- [IN-04] dry-checker を較正付き 2 段構成に変える: fast tier（軽量モデル）で全ペアを判定し、既知違反プローブの検出率で信頼性を較正する。較正が健全なら not-a-violation / accepted を確定し、violation および不正出力のみ重量級 tier へ escalation する。較正失敗時は全ペアを重量級 tier で再判定し、重量級でも較正失敗なら fail-closed でエスカレーション。fast tier の `fast_model` を `agent-profiles.json` に追加し、fast / final tier の reasoning effort とプローブ設定を `.harness/config/dry-check.json` に追加する。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D4] [tasks: T009, T011, T012, T013, T016]
- [IN-05] `sotp dry check-approved` の実装を変更し、埋め込み・類似検索を行わず、カバレッジ記録（`dry write` が記録した diff フラグメントの FragmentRef = path + content_hash）と dry-check.json の verdict 記録を読んで staleness（現在の diff フラグメントの FragmentRef がカバレッジ記録に在るか）と all-resolved（触れるペアが全て not-a-violation / accepted か）の 2 条件だけで Approved / Blocked を判定する。類似検索は `dry write`（resolver）にのみ残す。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D5] [tasks: T001, T003, T005]
- [IN-06] `dry write` がカバレッジ記録（処理した diff フラグメントの FragmentRef 集合）を dry-check.json または付随ファイルに記録し、`dry check-approved` の staleness 判定の入力として使えるようにする。同じ content_hash でも path が異なるフラグメントは別 FragmentRef として扱い、hash だけでは covered と見なさない。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D5] [tasks: T001, T004]
- [IN-07] テレメトリ計装の追加: D1 の効果測定のために no-fix completed 識別用 `GateEval`（gate_name=`dry`, verdict Approved→ok / Blocked→error）を追加できる。D4 の効果測定のために dry ラウンドに `round_type`（fast / final）+ tier 別 `model` タグを付与する `emit_review_round` 呼び出しを追加する。いずれも既存テレメトリ schema の範囲内で対応し、新フィールド・新イベント型は追加しない（既存の `GateEval` / `ReviewRound` と `round_type` / `model` タグを使う）。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D1, knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D4] [tasks: T007, T013]

### Out of Scope
- [OS-01] DFP と RFP の条件付き並列実行（dry violation が触る path 集合と review scope 集合の交差が空な場合に両フェーズを同時並列で走らせること）。本 ADR の低リスク改善で往復回数を削った後に実測して再検討する。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D1] [tasks: T010]
- [OS-02] per-task DFP の間引き（全コードベース gate を PR 前 1 回に限定する変更）。既存決定（`2026-06-02-0716-dry-checker.md` D7/D13）の改訂が必要なうえ、D1 により per-task DFP の定常コストが大幅に下がるため間引きの動機が弱い。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D1] [tasks: T006]
- [OS-03] 偽陽性ペアの再燃対策（内容の微変更でペア識別子が変わり、判定済み偽陽性ペアが再判定に回る問題の解消）および DRY 検証のシフトレフト（設計・実装フェーズへの DRY 検証前倒し）。別草案に分離して保留する。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D1] [tasks: T001]
- [OS-04] セマンティックインデックス構築・永続化・増分維持の設計変更。インデックス BUILD の最適化は `2026-06-04-1042-dry-checker-operability-and-batch-index.md` D6/D7 で決定済みであり、本トラックは索引の作り方を変えない。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D5, knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D6, knowledge/adr/2026-06-04-1042-dry-checker-operability-and-batch-index.md#D7] [tasks: T008]
- [OS-05] dry-checker / review / ref-verify の fix パス統合（DRY findings と review findings を 1 つの fix-lead に渡して 1 パスで直す）。`2026-06-02-0716-dry-checker.md` D12 で既に却下済みであり、本トラックで再検討しない。 [adr: knowledge/adr/2026-06-02-0716-dry-checker.md#D12] [tasks: T010]
- [OS-06] fixpoint 解決コマンド（D2）を既存の `bin/sotp track resolve`（lifecycle 用純関数）へ統合すること。両者は入力・cadence・失敗様式が異なる別コマンドとして設計する。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D2] [tasks: T015]
- [OS-07] blocking gate の意味論（`2026-06-02-0716-dry-checker.md` D7/D8）・全コードベース単一スコープ（D13）・修正役分離（D12）・verdict 書き込み経路（D10/D11）の変更。 [adr: knowledge/adr/2026-06-02-0716-dry-checker.md#D7, knowledge/adr/2026-06-02-0716-dry-checker.md#D12, knowledge/adr/2026-06-02-0716-dry-checker.md#D13] [tasks: T003]

## Constraints
- [CN-01] D1 の効率化後も、blocking gate の fail-closed 性（`2026-06-02-0716-dry-checker.md` D7）は変えない。`sotp dry check-approved` が Blocked のときは必ず judge が走り、未解決 violation は通過できない。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D1, knowledge/adr/2026-06-02-0716-dry-checker.md#D7] [tasks: T006]
- [CN-02] D2 の fixpoint 解決コマンドは各ゲートの公開インターフェース（exit code / 読み出し API）だけを合成し、dry-check.json / review.json / ref-verify cache の内部構造には依存しない（ゲート間の疎結合 `2026-06-02-0716-dry-checker.md` D1 を維持）。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D2, knowledge/adr/2026-06-02-0716-dry-checker.md#D1] [tasks: T014, T015]
- [CN-03] D3 の判定並列化は、1 ペア 1 agent 呼び出しの粒度を変えない（複数ペアを 1 呼び出しに詰める批量判定はしない）。エラーは収集・報告し、1 件の判定エラーで残りの判定結果を破棄しない。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D3] [tasks: T010]
- [CN-04] D3 の並列度上限は provider の rate limit / アカウント同時実行制限に合わせて運用側が調整できるよう `.harness/config/dry-check.json` の `max_parallelism` フィールドで設定可能にし、無制限並列は行わない。未指定なら nonzero の既定値を使用する。これは ref-verify が `max_parallelism` を `.harness/config/ref-verify.json` から読む実装と同じ分離方針に従う。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D3] [tasks: T008, T009, T010, T016]
- [CN-05] D3 の並列判定において、dry-check.json への記録は完了順ではなくペア識別子順で append し、dry-check.json の記録順を決定的に保つ。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D3] [tasks: T010]
- [CN-06] D4 の較正付き 2 段化において、既知違反プローブの verdict は dry-check.json の production 記録には残さない（プローブと production の記録を分離する）。プローブ injection rate・detection threshold 閾値は `.harness/config/dry-check.json` で調整可能にする。tier 別 model（`fast_model` / 既存 `model`）は `agent-profiles.json` の dry-checker capability に追加する。これは ref-verify の known-bad probe 設定（`known_bad_injection_rate_percent` / `known_bad_detection_threshold_percent` を `.harness/config/ref-verify.json` に置く方針）と同じ配置に従う。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D4, knowledge/adr/2026-05-27-1601-sot-chain-semantic-review-gate.md#D5] [tasks: T009, T011, T012]
- [CN-07] D4 の較正失敗時（fast tier のプローブ検出率が閾値未満）は fast 非違反判定を信頼せず、全ペアを重量級 tier で再判定する。重量級 tier でもプローブ検出率が閾値未満なら fail-closed で人間にエスカレーションし、gate を通過させない。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D4] [tasks: T012]
- [CN-08] D5 の純読み取りゲート化において、カバレッジ記録の欠落・不整合時は fail-closed（Blocked）で安全側に倒す。カバレッジ記録は FragmentRef（path + content_hash）単位で照合し、同一 content_hash の別 path を covered と誤認しない。ゲートは自己完結性を手放し `dry write` が記録したカバレッジ記録の存在に依存することを受け入れる。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D5] [tasks: T001, T002, T003, T004, T005]
- [CN-09] D5 の `dry check-approved` は staleness 判定を diff フラグメントの FragmentRef（path + content_hash）とカバレッジ記録の FragmentRef 照合で行い、埋め込み・類似検索を呼ばない。これにより `check-approved` は何度実行しても実害がなくなり、D1/D2 の組み合わせで二重実行が生じても問題にならない。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D5] [tasks: T003, T005]
- [CN-10] 効果計測は既存テレメトリ（wall-clock `duration_ms` / 構造カウント `findings_count` / round 数 / subprocess 数）で行い、トークン建て絶対コストは測定対象としない。`cargo make ci-rust` はテレメトリに出ないため、D1 の効果は dfl run の総 wall-clock 崩落で代理測定する。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D1] [tasks: T007, T013]

## Acceptance Criteria
- [ ] [AC-01] `.claude/agents/dry-fix-lead.md` が変更され、violation ゼロ時に `cargo make ci-rust` と 2 度目の `sotp dry write` をスキップして `completed` に到達するループ分岐を持つ。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D1] [tasks: T006]
- [ ] [AC-02] violation ゼロで終わる dfl run の wall-clock span が `cargo make ci-rust`（フル CI、分単位）を含まない floor（`dry write` ≤ 1 回 + check のみ）に収束することをテレメトリ（dfl の dry ReviewRound 群の `duration_ms`）で確認できる、または no-fix completed を識別する `GateEval`（gate_name=`dry`）が発行されることで確認できる。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D1] [tasks: T006, T007]
- [ ] [AC-03] `sotp track fixpoint-resolve`（仮称）または相当のコマンドが存在し、dry gate / review per-scope 状態 / ref-verify gate の 3 ゲートを合成した次フェーズ（DFP / RFP(scope list) / ref-verify / commit）を決定的に出力する。orchestrator はこの出力に従うだけで収束判定ロジックを持たない。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D2] [tasks: T014, T015]
- [ ] [AC-04] fixpoint 解決コマンドが各ゲートの公開 API（exit code / 読み出し API）のみを使い、dry-check.json / review.json / ref-verify cache の内部構造を直接読まないことを review で確認できる。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D2] [tasks: T014, T015]
- [ ] [AC-05] `sotp dry write` が 2 相（照会相 → 判定相）で動き、判定相が設定可能な `max_parallelism` 上限（`.harness/config/dry-check.json`）で並列 fan-out を行う。dry-check.json への記録がペア識別子順で append される（記録順が決定的）。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D3] [tasks: T010, T016]
- [ ] [AC-06] 未検証ペアが複数存在する run において、`sotp dry write` の wall-clock が（並列度を 1 超に設定した場合）直列実行ベースラインより短縮されることをテレメトリ（dry ReviewRound の `duration_ms`）で確認できる。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D3] [tasks: T010, T017]
- [ ] [AC-07] dry-checker が fast tier（軽量モデル、`agent-profiles.json` の `fast_model`）と重量級 tier（既存 `model`）の 2 段で動く。fast tier は production ペアとは別の 1 ペア 1 agent 呼び出しで既知違反プローブを判定してプローブ検出率を較正し、較正が健全なら not-a-violation / accepted を確定し、violation および不正出力のみ重量級 tier へ escalation する。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D4] [tasks: T011, T012]
- [ ] [AC-08] 較正失敗時（fast tier プローブ検出率 < threshold）に全ペアを重量級 tier で再判定し、重量級でも較正失敗なら gate が fail-closed になることを unit または integration test で確認できる。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D4] [tasks: T012]
- [ ] [AC-09] テレメトリに dry ラウンドの `round_type`（fast / final）と tier 別 `model` が記録され、fast 完結率（同 scope で final 後続なし / fast round 数）と昇格率（final / fast round 数）を後から集計できる。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D4] [tasks: T013]
- [ ] [AC-10] `sotp dry check-approved` の実装が埋め込み・類似検索を行わず、カバレッジ記録（diff フラグメントの FragmentRef = path + content_hash）と dry-check.json の verdict 記録だけを読んで staleness + all-resolved を判定する。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D5] [tasks: T003, T005]
- [ ] [AC-11] `sotp dry write` がカバレッジ記録（処理した diff フラグメントの FragmentRef 集合）を記録し、その記録を `dry check-approved` が staleness 判定に使う。フラグメント編集または path 変更 → FragmentRef 変化 → カバレッジ記録に不在 → Blocked → `dry write` 強制、という意味論が維持される。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D5] [tasks: T004, T005]
- [ ] [AC-12] `dry check-approved` を繰り返し呼んでも安価（埋め込み・検索なし）であり、D1 の dfl ループ先頭 check と D2 の fixpoint 合成が同一の `dry check-approved` を重複実行しても実害がない。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D5] [tasks: T003, T005, T007]
- [ ] [AC-13] 実装完了後、`cargo make ci`、`bin/sotp review check-approved`、`bin/sotp dry check-approved` が pass する。 [adr: knowledge/adr/2026-06-10-0413-dfp-rfp-loop-cost-reduction.md#D1] [tasks: T017]

## Related Conventions (Required Reading)
- knowledge/conventions/dry-check-workflow.md#2. DFP → RFP の 2 フェーズ実行順序
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- .claude/rules/04-coding-principles.md#Make Illegal States Unrepresentable
- .claude/rules/04-coding-principles.md#No Panics in Library Code

## Signal Summary

### Stage 1: Spec Signals
🔵 42  🟡 0  🔴 0

