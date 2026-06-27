<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 29, yellow: 0, red: 0 }
---

# タスク単位の契約履行 pre-review ゲート

## Goal

- [GO-01] 型カタログと実装の構造的一致（impl_catalog 信号）を確認する最初の hard gate を、現行の merge-gate（strict）から reviewer 入場前（per-task）へ shift-left する。契約違反（宣言シンボル欠落・shape ずれ）を authoring 直後・per-task で表面化させ、merge 直前まで遅延する手戻りコストを除去する。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D2]
- [GO-02] 新 Phase 3 artifact `task-contract.json` を導入し、タスク → 型カタログ entry の attribution を complete relation として保持する。impl-planner（Phase 3 writer）が本 artifact を author し、どのタスクがどの entry の履行責務を持つかを明記する。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D1]
- [GO-03] ゲートの合否基準は impl_catalog 信号 🔵 のみとし、body の liveness（`todo!()` でないか等）はゲートの管轄外として reviewer の責務に留置する。「構造的整合 = 機械的信号 / 意味論的整合 = LLM レビュー」という既存の責任分界点を維持する。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D3]
- [GO-04] ゲート通過時に verified-conformance サマリを reviewer briefing へ追記し、ゲートの保証範囲（「宣言した API surface が型契約と shape 一致、body は未検証」）を正確に伝える。過大表現による reviewer の over-trust とすり抜け悪化を防ぐために文言を規律する。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D5]

## Scope

### In Scope
- [IN-01] 新 Phase 3 artifact `task-contract.json` のスキーマ定義と codec 実装。artifact はタスクリスト（各タスクが履行責務を持つ型カタログ entry 識別子のリスト）と schema_version を含む。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D1] [tasks: T001, T003]
- [IN-02] impl-planner の Phase 3 output 責務拡張: impl-plan.json・task-coverage.json に加えて `task-contract.json` を author する。type-designer（Phase 2）の責務は変更しない。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D1] [tasks: T001]
- [IN-03] attribution の complete relation invariant 検証: (a) review scope 内の関連カタログ entry が全て ≥1 タスクに attribution されていること、(b) orphan entry（review scope 内だがどのタスクにも attribution されていない関連 entry）が存在しないこと、(c) attribution された entry がカタログに実在し当該 scope 内であること（referential integrity）。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D1] [tasks: T002]
- [IN-04] pre-review blocking binary check ゲートの実装: task-contract.json の attribution completeness を検証した後、attribution された全カタログ entry の impl_catalog 信号を既存信号テーブルと entry キーで JOIN し、全 entry が 🔵 かを確認する。ゲートは sotp CLI サブコマンドとして実装し、fail-closed（artifact 不在・attribution 不完全・非 🔵 entry のいずれでもブロック）とする。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D2, knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D4] [tasks: T001, T002, T003, T004, T005, T006]
- [IN-05] review-fix ワークフロー（`sotp review fix-local` / fixpoint_resolve または同等の呼び出し経路）への pre-review ゲートの配線。ゲートは `sotp review fix-local` が reviewer invocation を起動する前に実行され、ゲートがブロックした場合は reviewer invocation が実行されない。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D2] [tasks: T008]
- [IN-06] ゲート通過時の verified-conformance サマリ生成と reviewer briefing への追記。文言は「宣言した API surface が型契約と shape 一致（body は未検証 — stub / liveness は reviewer が確認）」とし、D5 の文言規律に従う。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D2, knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D5] [tasks: T002, T006, T008]

### Out of Scope
- [OS-01] body の liveness / stub 検査（test-pass・stub-scan・syn による body walking）。ゲートの直接入力は既存 impl_catalog / type-signals document であり、上流の impl_catalog 信号は rustdoc JSON（body を含まない）から生成されるため、🔵 判定は宣言シンボルの存在と shape 一致のみを保証する。stub が通過することは意図的なスコープ限定である。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D3]
- [OS-02] 新 SoT Chain（task_conformance 等）の追加。ゲートは binary check として実装し、既存 impl_catalog 信号を再利用する。chain matrix・strictness 設定・views の表面積は増やさない。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D4]
- [OS-03] 型カタログ（`<layer>-types.json`）へのタスク情報の追加。Phase 2 成果物に Phase 3 概念（task）への後方依存を持たせることは SoT Chain 逆流であり禁止。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D1]
- [OS-04] `task-coverage.json`（spec coverage artifact）への attribution 情報の追加。参照 SSoT・供給先ゲート・invariant がいずれも異なる別責務 artifact とし、同居させない。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D1]
- [OS-05] pre-review ゲートの advisory モード・緩和モード（per-task / per-track のブロック回避オプション）。ゲートは blocking かつ fail-closed で実装する。interim 的運用の緩和は本 track 対象外。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D2]
- [OS-06] SignalEvaluatorV2（既存の型カタログ ↔ 実装比較エンジン）の変更。ゲートは本エンジンの評価結果（per-entry impl_catalog 信号）を読み取るだけであり、エンジン自体は変更しない。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D4]

## Constraints
- [CN-01] attribution は review scope 内の全関連カタログ entry に対する complete relation でなければならない。orphan entry（scope 内で attribution されていない関連 entry）が存在する場合、その entry の impl_catalog 信号がゲート入力から漏れ shift-left の保証が崩れるため、ゲートは orphan 検出時に明示的にブロックする。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D1] [tasks: T002]
- [CN-02] task-contract.json が参照するカタログ entry 識別子は現行カタログに実在し当該 scope 内でなければならない（referential integrity）。存在しない entry や scope 外 entry への attribution はゲートエラーとし、stale な task-contract.json の検出手段とする。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D1] [tasks: T002]
- [CN-03] ゲートの合否基準は impl_catalog 信号 🔵 のみ。🟡（Yellow）は 🔴（Red）と同様に blocking とする。body の確認・test 結果・stub 走査は判定に含めない。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D3] [tasks: T002]
- [CN-04] reviewer briefing の verified-conformance 行は「宣言した API surface が型契約と shape 一致（body は未検証 — stub / liveness は reviewer が確認）」と正確に記述する。「契約 satisfied」「実装済み」「liveness 確認済み」等の過大な表現は使用しない。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D5] [tasks: T002, T007]
- [CN-05] pre-review ゲートは既存 impl_catalog 信号テーブルを entry キーで JOIN して動作し、新 chain・新信号エンジン・型カタログ拡張を必要としない。SignalEvaluatorV2 は変更せずに再利用する。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D4] [tasks: T002, T003]
- [CN-06] task-contract.json の author は impl-planner（Phase 3）に限定する。type-designer（Phase 2）は本 artifact を書かない。Phase 2 成果物が Phase 3 概念（task）を参照することは SoT Chain の順序違反であり禁止。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D1] [tasks: T001]

## Acceptance Criteria
- [ ] [AC-01] `task-contract.json` のスキーマが定義されており、impl-planner が Phase 3 output として本 artifact を author する。artifact には schema_version・track id・タスクリスト（各タスクが履行責務を持つ型カタログ entry 識別子のリスト）が含まれる。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D1] [tasks: T001, T003]
- [ ] [AC-02] attribution の complete relation invariant が機械的に検証可能である: (a) review scope 内の関連カタログ entry が全て ≥1 タスクに attribution されている、(b) orphan entry が存在しない、(c) attribution 対象 entry がカタログに実在し scope 内である。invariant 違反時にゲートが非ゼロ終了コードで返り、違反内容（orphan entry リスト・存在しない entry 識別子）を出力する。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D1] [tasks: T002, T007]
- [ ] [AC-03] pre-review ゲートのコマンド（`sotp` CLI サブコマンドまたは Makefile.toml タスク）が追加されており、review-fix ワークフロー内で `sotp review fix-local` が reviewer invocation を起動する前に実行される。task-contract.json が不在の場合、ゲートはファイル不在を明示するエラーメッセージで失敗する。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D2] [tasks: T004, T005, T006, T008]
- [ ] [AC-04] attribution された全カタログ entry の impl_catalog 信号が 🔵 のとき、ゲートがパスして verified-conformance サマリを生成する。attribution された entry に impl_catalog 信号 🟡 または 🔴 のものが含まれる場合、ゲートがブロックし、非 🔵 entry の一覧と信号状態を出力する。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D2, knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D3] [tasks: T002, T007]
- [ ] [AC-05] reviewer briefing へ追記される verified-conformance サマリが「宣言した API surface が型契約と shape 一致（body は未検証 — stub / liveness は reviewer が確認）」の文言規律に従っており、「契約 satisfied」「実装済み」等の過大表現を含まない。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D5] [tasks: T002, T007, T008]
- [ ] [AC-06] ゲートがブロックした場合、`sotp review fix-local` 経由の reviewer invocation が実行されない。ゲートは fail-closed であり、ゲートエラーが上位ワークフローに伝播して review invocation を中断する。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D2] [tasks: T008]
- [ ] [AC-07] `cargo make ci`（fmt-check + clippy + nextest + deny + check-layers + verify-*）が pass する。ゲートロジックのユニットテストとして次のケースを網羅する: (a) task-contract.json 不在でブロック、(b) orphan entry 存在でブロック、(c) attribution referential integrity 違反でブロック、(d) 非 🔵 entry 存在でブロック、(e) 全 attributed entry 🔵 かつ attribution 完全でパス・verified-conformance サマリ生成。既存テストへのリグレッションがない。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D2] [tasks: T007]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#No Panics in Library Code
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule
- knowledge/conventions/workflow-ceremony-minimization.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 29  🟡 0  🔴 0

