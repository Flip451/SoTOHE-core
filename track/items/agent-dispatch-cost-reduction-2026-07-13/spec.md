<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 41, yellow: 0, red: 0 }
---

# 外部 agent 呼び出しのコスト削減

## Goal

- [GO-01] provider CLI subprocess を起動する capability の実行 effort を capability ごと（reviewer 系は fast/final ごと）に profile で明示し、低コストの fast round と完全な final 判定の二段構成を維持したまま、暗黙の provider 既定への依存をなくす。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D1]
- [GO-02] 同一 scope・同一 round 種別の reviewer 再判定と、同一成果物への capability 継続作業で安全に session を再開できるようにし、再入ごとの文脈再構築コストを削減する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D2, knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D4]
- [GO-03] reviewer 起動前の signal 前段処理で、対象 crate の source 内容・依存解決（lockfile）・toolchain 識別子から得る実装側入力 hash を per-layer artifact の記録値と比較し、判定材料が変わる評価を省略しないまま不要な rustdoc 抽出の再計算コストを削減する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D3]
- [GO-04] すべての最適化経路を保守的に失効・再計算・新規 session へ倒し、既存の review judgment、review record、および signal の正確性を犠牲にしない。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D2, knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D3, knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D4]

## Scope

### In Scope
- [IN-01] capability routing profile に、provider CLI subprocess を起動する全 capability の effort を宣言する。reviewer-like capability は fast/final ごとに宣言し、default profile の reviewer fast は low、final は各 provider の最大段階とする。semantic reference 検証の ref-verifier-chain1 / ref-verifier-chain2 も対象に含める。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D1] [tasks: T001, T002, T004, T007]
- [IN-02] 対象 capability（reviewer は round 種別ごと）の effort が profile から解決できない dispatch を拒否し、provider の暗黙既定 effort で実行しない。hosted service 側で実行される pr-reviewer は effort 宣言とこの拒否検査の対象外とする。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D1] [tasks: T001, T002, T007]
- [IN-03] fix 後の同一 scope・同一 round 種別の reviewer 再 round で、track・scope・round 種別に結び付く prior session を resume し、scope の file list と diff を reviewer 自身が確認して全件を再判定する。初回 round と fast から final への escalation は新規 session とする。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D2] [tasks: T003, T004, T009]
- [IN-04] reviewer session cache を provider と model に束縛する。現在の profile 解決結果の provider または model が記録値と不一致、resume 失敗、または session 期限切れなら、cache を使わず新規 session を開始する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D2] [tasks: T003, T004]
- [IN-05] reviewer 起動 wrapper は layer（crate）ごとに、対象 crate の source 内容・依存解決（lockfile）・toolchain 識別子から得る実装側入力 hash を機械的に判定する。実装側入力が不変なら rustdoc 抽出を skip し、catalogue または spec の変更は signal を再評価するが、rustdoc 抽出の再実行要否とは分離して判定する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D3] [tasks: T005, T011, T013, T014, T012, T015, T016, T018]
- [IN-06] catalogue または spec の変更時は signal を再評価するが、実装側入力 hash が artifact 記録値と一致するときは rustdoc 抽出を再実行しない。実装側入力 hash が不一致または判定不能なら、rustdoc 抽出から再計算する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D3] [tasks: T005, T011, T013, T014, T017, T015, T016, T018]
- [IN-07] per-layer type-signals artifact 自体に、既存の declaration_hash と対称に実装側入力 hash を記録する。artifact に記録された declaration_hash と実装側入力 hash を現在の対応する入力と比較して鮮度を判定し、必要な hash が使えない場合は保守的に再計算する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D3] [tasks: T005, T011, T013, T018]
- [IN-08] `sotp capability exec` に orchestrator が選ぶ resume option を追加する。同一 track・同一 capability の継続作業（同じ成果物への追補・修正の再入、中断からの続行）を対象とし、初回 dispatch と関心事を切り替える dispatch は orchestrator の判断で新規 session とする。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D4] [tasks: T003, T007, T008, T009]
- [IN-09] capability resume cache を、track 内では track・capability、track 外では workspace transient 配下の capability・対象 artifact の repo-relative path で分離する。対象 path が未確定の track 外 dispatch は cache を使わず記録もしない。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D4] [tasks: T003, T007, T008]
- [IN-10] obligation gate の check と evaluate の判定対称性を修理する。edge 所有述語と waiver 優先規則を domain の obligation / binding model に置き、両 gate は同一述語を消費する。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D2, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D4, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D9] [tasks: T019]

### Out of Scope
- [OS-01] fast/final の二段 review 構成を廃止または統合すること、または final round を fast の session から resume することは含めない。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D1, knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D2] [tasks: T004]
- [OS-02] repository 外の hosted service で実行される pr-reviewer への model・effort 注入は含めない。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D1] [tasks: T002, T007]
- [OS-05] in-host subagent delegation の resume 対応は含めない。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D4] [tasks: T007]
- [OS-03] session id を committed SoT file、review record、track identity に保存すること、または新しい top-level transient path を作ることは含めない。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D2, knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D4] [tasks: T003, T005, T007]
- [OS-04] D3 の freshness 状態を per-layer type-signals artifact と別の cache file に保存することは含めない。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D3] [tasks: T005, T011, T013, T018]

## Constraints
- [CN-01] 対象 capability の effort が未指定の dispatch を暗黙の provider 既定で実行せず、dispatch を拒否する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D1] [tasks: T001, T002, T007]
- [CN-06] reviewer session の記録済み provider または model が現在の profile 解決結果と不一致、resume 失敗、または期限切れなら、stale な reuse をせず新規 session で round を実行する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D2] [tasks: T003, T004]
- [CN-07] 実装側入力 hash を判定できない場合は、signal 評価の skip をせず rustdoc 抽出から再計算する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D3] [tasks: T005, T011, T013, T014, T016, T018]
- [CN-11] type-signals 評価の書込み対象は、現在 branch と照合済みの track identity に束縛し、不一致は拒否する。 [adr: knowledge/adr/2026-05-26-1813-track-id-default-active-track.md#D7] [tasks: T011]
- [CN-08] capability resume cache は provider と model に束縛する。現在の profile 解決結果の provider または model が記録値と不一致、resume 失敗、または期限切れなら、stale な reuse をせず新規 session で dispatch を実行する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D4] [tasks: T003, T007]
- [CN-02] resume 時も dispatcher は現在の profile と provider-native sandbox 定義を通常どおり解決し、model・sandbox・effort を含む全実行 flag を毎回明示的に再注入する。provider の session 設定継承に依存しない。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D2, knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D4] [tasks: T001, T004, T007]
- [CN-03] reviewer は resume 後も scope 全件を再判定する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D2] [tasks: T004, T009]
- [CN-09] session resume は review judgment と review record の単位を変更しない。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D2] [tasks: T004]
- [CN-10] writer 系 capability は resume 後に上流 artifact の変更有無を自ら確認し、変更があれば再読してから作業する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D4] [tasks: T007, T008, T009]
- [CN-04] session cache は gitignored machine-local transient とし、track 解決時は track artifact directory 配下、track 解決不能時は既存 workspace runtime path 配下に置く。cache の lifecycle と key は track または対象 artifact の境界を越えて混線しない。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D2, knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D4] [tasks: T003, T007, T008]
- [CN-05] D3 の reuse 判定は、対象 crate の source 内容、依存解決（lockfile）、toolchain 識別子から得る content hash に基づく。判定粒度は layer（crate）単位とし、変更がある layer だけを再計算する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D3] [tasks: T005, T011, T013, T012, T016, T018]

## Acceptance Criteria
- [ ] [AC-01] default routing profile は subprocess を起動する対象 capability の effort を解決でき、reviewer-like capability では fast と final の別 effort が解決できる。default profile の reviewer fast は low、final は各 provider の最大段階であり、fast の前置判定と final の完全判定から成る二段 review を維持する。ref-verifier-chain1 / ref-verifier-chain2 は対象であり、pr-reviewer は対象外である。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D1] [tasks: T001, T002, T004, T007]
- [ ] [AC-02] 対象 capability の effort が欠ける dispatch は provider の既定値で起動せず拒否され、pr-reviewer は effort 未指定でもこの拒否検査の対象にならない。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D1] [tasks: T001, T002, T007]
- [ ] [AC-03] 同一 track・scope・round 種別で fixer 後に再実行した reviewer round は、現在の profile 解決結果の provider と model が一致する有効な prior session を resume し、全実行 flag を明示的に再指定したうえで reviewer は現在の file list と diff から scope 全件を再判定する。resume は文脈だけを再利用し、その round の judgment と review record は新規 session の round と同じ単位・意味で記録される。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D2] [tasks: T003, T004, T009]
- [ ] [AC-04] reviewer の初回 round、fast から final への escalation、profile 解決結果の provider または model の不一致、resume 失敗、または session 期限切れでは、新規 session で round を実行する。resume・新規のどちらでも model・sandbox・effort の全 flag は明示指定される。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D2] [tasks: T003, T004]
- [ ] [AC-05] rustdoc 抽出の要否は、layer の artifact に記録された implementation-side input hash と現在の対応する input hash の比較だけで判定する。一致時は rustdoc 抽出を skip し、不一致または判定不能時は AC-07 の条件に従って rustdoc 抽出から再計算する。declaration_hash または spec の差分は signal 再評価の要否だけを決め、両方が一致するときは評価ごと skip し、declaration_hash のみが不一致で implementation-side input hash が一致するときは AC-06 の条件に従って rustdoc を起動せず signal を再評価する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D3] [tasks: T015, T005, T011, T013, T014, T012, T016, T018]
- [ ] [AC-06] catalogue または spec だけが変わった layer は、implementation-side input hash が一致するとき、rustdoc を起動せずに signal を再評価する。rustdoc 抽出の再実行要否と signal 再評価要否は入力ごとに分離して判定する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D3] [tasks: T015, T005, T011, T013, T014, T017, T016, T018]
- [ ] [AC-07] implementation-side input hash の不一致または判定不能では、rustdoc 抽出から再計算する。判定と再計算は layer（crate）単位とし、変更がある layer だけを再計算する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D3] [tasks: T005, T011, T013, T014, T016, T018]
- [ ] [AC-08] per-layer type-signals artifact は、D3 の freshness 情報として計算結果と同じ artifact に declaration_hash と実装側入力 hash を記録し、必要な hash が欠ける artifact を reuse 可能と誤認しない。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D3] [tasks: T005, T013, T018]
- [ ] [AC-09] orchestrator が継続作業と判断して resume option を指定した provider CLI subprocess dispatch の `sotp capability exec` は、track 内では同一 track・capability の有効 cache entry を再開する。track 解決時は track-local cache、track 解決不能時は capability・対象 artifact の repo-relative path による workspace-local cache を選び、初回 dispatch、関心事を切り替えた dispatch、対象 path 未確定の track 外 dispatch、または provider・model 不一致では新規 session で実行する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D4] [tasks: T003, T007, T008]
- [ ] [AC-10] capability resume の再開失敗または session 期限切れは dispatch を中断させず、新規 session に fallback する。resume capability は上流 artifact の変更有無を確認し、変更があれば再読し、全実行 flag を再注入したうえで作業する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D4] [tasks: T003, T007, T008, T009]
- [ ] [AC-11] waiver を持つ edge は fulfillment または自発的 binding から adjudication されず、edge を所有する全 obligation は (edge × obligation) 対ごとに独立して解決される。check と evaluate はこの規則から同一の帰結を得る。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D2, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D4, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D9] [tasks: T019]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Error Handling: Result and ? Operator
- knowledge/conventions/coding-principles.md#No Panics in Library Code
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/responsibility-boundary.md#Rules
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/track-lifecycle.md#Generated Views
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies

## Signal Summary

### Stage 1: Spec Signals
🔵 41  🟡 0  🔴 0

