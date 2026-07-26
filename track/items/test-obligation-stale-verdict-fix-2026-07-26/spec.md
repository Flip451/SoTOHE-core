<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 22, yellow: 0, red: 0 }
---

# テスト義務 fulfillment cache の stale verdict 誤判定を解消する ADR 準拠回復

## Goal

- [GO-01] `sotp test-obligation evaluate` と `sotp test-obligation check` が、同じ fulfillment verdict cache entry を同じ完全な同一性・有効性規律で扱い、正しい現行 verdict を古い別の binding 行によって Stale と誤報しないようにする。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D6, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D16]
- [GO-02] cache の不整合、義務を持つ edge への不正な自発的 binding、および評価エラーを黙殺せず、既存の fail-closed gate を自己ホスト可能な回復経路のまま維持する。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D11, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D16]

## Scope

### In Scope
- [IN-01] fulfillment verdict の cache reader は、bound tests 集合 hash・entry 宣言 hash・anchor hash の三つ組と現在の verifier-prompt fingerprint を一貫して照合する。`evaluate` の再利用判定と `check` の freshness 判定は、同一 edge に複数の過去行があっても同じ現行 entry を認識する。`evaluate` が verifier provider の解決不能を受けた場合は VerifierPort error を呼出元へ伝播し、成功した評価または current fulfilled verdict として扱わない。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D6, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D16] [tasks: T001, T002]
- [IN-02] fulfillment cache 行の同一性を完全な cache key により定める。同じ完全な key を持つ複数行は曖昧な cache として明示的に検出し、edge id と obligation id だけによる first-match 選択で verdict を採用しない。異なる hash 三つ組を持つ同一 edge・obligation の過去行は、現行 key に一致しない限り current verdict として扱わない。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D6, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D16] [tasks: T001, T002]
- [IN-03] 実在する導出義務を所有する edge に `voluntary_binding` を直接記録した状態を、binding の不整合として機械的に検出する。義務なし edge のためだけに許された自発的 binding を、義務あり edge の fulfillment 解決として黙って無視または受理しない。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D2, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D9, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D11] [tasks: T003]
- [IN-04] `evaluate` の結果 entry から非正本の `entries[].verdict_reason` を削除し、失敗理由の正本を `entries[].verdict.reason` に限定する。失敗 verdict ではこの nested field を必須かつ非空とし、null placeholder、alias、または mirror を設けない。 [adr: knowledge/adr/2026-07-26-1505-test-obligation-evaluate-verdict-reason-removal.md#D1] [tasks: T004]
- [IN-05] fulfillment cache 行に、hash の根拠となった resolved `bound_tests` を診断専用情報として記録する。この情報は調査可能性を高めるが、cache key の同一性軸を増やさず、bound tests 集合 hash を含む既存の三つ組規律を変更しない。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D6, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D9] [tasks: T002, T004]

### Out of Scope
- [OUT-01] `check` または `evaluate` に force、lenient、手動 cache purge を代替する再計算フラグを追加しない。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D11, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D16] [tasks: T002]
- [OUT-02] verdict cache の手動削除または手編集を、stale verdict からの正規回復手順にしない。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D16] [tasks: T002]
- [OUT-03] 義務導出規則、義務 id・edge id の安定性、または義務なし edge に対する waiver / voluntary binding の三形態を再設計しない。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D2, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D9] [tasks: T003]
- [OUT-04] waiver verdict cache の重複処理、task-status lane の解釈、または verdict cache key を構成する三つ組そのものを変更しない。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D6, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D16] [tasks: T001, T004]

## Constraints
- [CN-01] `check` は pure-read の fail-closed gate のままとし、現行 key に一致しない行、fingerprint が不一致または不在の行、または曖昧な同一 key 行を現行 fulfilled verdict として採用してはならない。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D6, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D11, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D16] [tasks: T001]
- [CN-02] キャッシュの過去行が残る場合でも、現行 binding と現行の hash / fingerprint に対応する fulfilled verdict がある限り、行の格納順序によって `check` が StaleVerdicts を報告してはならない。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D6, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D16] [tasks: T001, T002]
- [CN-03] 義務あり edge の `voluntary_binding`、同一 key の矛盾行、または解決不能な cache / binding 不整合は、warning だけで通過させず finding と非零 exit で失敗させる。治療は binding または `evaluate` による正規の再評価で行う。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D9, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D11] [tasks: T001, T002, T003]
- [CN-04] 診断情報は、fulfillment cache 行の resolved `bound_tests` と、失敗 `evaluate` entry の唯一かつ非空の `entries[].verdict.reason` に限定する。これらは観測用であり、current verdict の選択は D6 の hash 三つ組と D16 の verifier-prompt fingerprint だけで決まり、診断情報による別の cache key または採用経路を作らない。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D6, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D16, knowledge/adr/2026-07-26-1505-test-obligation-evaluate-verdict-reason-removal.md#D1] [tasks: T002, T004]

## Acceptance Criteria
- [ ] [AC-01] 同じ edge・obligation に対して、過去の bound tests 集合 hash の fulfilled 行と、現行 binding の hash の fulfilled 行が共存する fixture で検証する。`evaluate` の cache lookup と `check` の freshness verification はいずれも現行の三つ組と fingerprint に一致する行を選び、行の順序にかかわらず `check` は StaleVerdicts を報告せず成功する。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D6, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D16] [tasks: T001, T002]
- [ ] [AC-02] 同じ完全な cache key に複数の fulfillment cache 行があり verdict 内容が矛盾する fixture で、`check` が first-match で一方を採用せず、cache の曖昧性を finding として表示して非零 exit することを検証する。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D6, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D11] [tasks: T001]
- [ ] [AC-03] 導出済み義務を所有する edge へ `voluntary_binding` を置いた fixture で、`check` が当該 binding を黙殺せず、義務あり edge には voluntary binding を使えないことを示す finding と非零 exit を返すことを検証する。義務なし edge の自発的 binding は引き続き fulfillment 評価へ進めることも検証する。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D2, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D9, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D11] [tasks: T003]
- [ ] [AC-04] `evaluate` の失敗 verdict fixture で、出力 schema、domain model、および encoder が `entries[].verdict_reason` を表現または出力せず、対応する `entries[].verdict.reason` が唯一の理由として必須かつ非空であることを検証する。 [adr: knowledge/adr/2026-07-26-1505-test-obligation-evaluate-verdict-reason-removal.md#D1] [tasks: T004]
- [ ] [AC-05] `evaluate` が書く fulfillment cache 行に resolved `bound_tests` が診断情報として記録されることを検証する。cache key の一致判定は `bound_tests` 表現ではなく、bound tests 集合 hash・entry 宣言 hash・anchor hash と fingerprint により決まり、診断情報を key の軸に含めないことも検証する。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D6, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D9, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D16] [tasks: T002, T004]
- [ ] [AC-06] 旧形式または stale な fulfillment cache 行を含む active track fixture で、初回の `check` が必要な finding を fail-closed に返し、`evaluate` による現行 verdict の正規再評価後の `check` が、手動 cache 編集・削除または force / lenient flag なしに成功することを検証する。これにより、この gate 自身を通る実装 batch の回復経路を維持する。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D6, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D11, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D16] [tasks: T002]
- [ ] [AC-07] verifier provider が解決不能となる fixture で、`evaluate` が VerifierPort error を呼出元へ返すことを検証する。この失敗を成功した評価または current fulfilled verdict として扱わないことも検証する。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D6] [tasks: T002]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Error Handling: Result and ? Operator
- knowledge/conventions/coding-principles.md#No Panics in Library Code
- knowledge/conventions/testing.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/no-backward-compat.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 22  🟡 0  🔴 0

