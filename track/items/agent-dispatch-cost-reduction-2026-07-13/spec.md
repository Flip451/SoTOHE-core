<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 38, yellow: 0, red: 0 }
---

# 外部 agent 呼び出しのコスト削減

## Goal

- [GO-01] provider CLI subprocess を起動する capability の実行 effort を capability ごと（reviewer 系は fast/final ごと）に profile で明示し、低コストの fast round と完全な final 判定の二段構成を維持したまま、暗黙の provider 既定への依存をなくす。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D1]
- [GO-02] 同一 scope・同一 round 種別の reviewer 再判定と、同一成果物への capability 継続作業で安全に session を再開できるようにし、再入ごとの文脈再構築コストを削減する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D2, knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D4]
- [GO-03] reviewer 起動前の signal 前段処理で、検証済みの同一実装入力と rustdoc-extraction contract に限って rustdoc 抽出を再利用し、判定材料が変わる評価を省略しないまま不要な再計算コストを削減する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D3]
- [GO-04] すべての最適化経路を保守的に失効・再計算・新規 session へ倒し、既存の review judgment、review record、および signal の正確性を犠牲にしない。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D2, knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D3, knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D4]

## Scope

### In Scope
- [IN-01] capability routing profile に、provider CLI subprocess を起動する全 capability の effort を宣言する。reviewer-like capability は fast/final ごとに宣言し、default profile の reviewer fast は low、final は各 provider の最大段階とする。semantic reference 検証の ref-verifier-chain1 / ref-verifier-chain2 も対象に含める。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D1] [tasks: T001, T002, T004, T007]
- [IN-02] 対象 capability（reviewer は round 種別ごと）の effort が profile から解決できない dispatch を拒否し、provider の暗黙既定 effort で実行しない。hosted service 側で実行される pr-reviewer は effort 宣言とこの拒否検査の対象外とする。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D1] [tasks: T001, T002, T007]
- [IN-03] fix 後の同一 scope・同一 round 種別の reviewer 再 round で、track・scope・round 種別に結び付く prior session を resume し、scope の file list と diff を reviewer 自身が確認して全件を再判定する。初回 round と fast から final への escalation は新規 session とする。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D2] [tasks: T003, T004, T009]
- [IN-04] reviewer session cache を provider、model、および capability SSoT・scope-specific severity policy・review-scope 設定・固定 execution-contract briefing 部から得る安定 fingerprint に束縛する。profile 解決結果または fingerprint の不一致、読出し・hash 化不能、resume 失敗、または期限切れでは cache を使わず新規 session を開始する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D2] [tasks: T003, T004]
- [IN-05] reviewer 起動 wrapper は layer ごとに、実装側 build-input closure、catalogue declaration、baseline、evaluator contract、rustdoc-extraction contract、および検証済み live rustdoc snapshot の一致を機械的に判定する。すべての signal 評価入力が不変なときだけ評価を skip し、catalogue・baseline・evaluator 側の変化では必要な評価を実行する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D3] [tasks: T005, T011, T012, T013, T015, T016, T017, T018]
- [IN-06] catalogue または baseline の変更時は、現行実装側 input と rustdoc-extraction contract が artifact 記録値に一致し、解決済み target の live rustdoc JSON を安全に読出し・parse でき、その内容 hash が記録済み snapshot hash と一致するときだけ rustdoc 抽出を省略する。rustdoc-extraction contract の変更・不明、実装側 input の不一致・不明、または snapshot 検証不能では rustdoc 抽出から再計算する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D3] [tasks: T005, T011, T012, T013, T014, T015, T016, T017, T018]
- [IN-07] per-layer type-signals artifact を schema version bump し、既存 declaration hash と対称に implementation-side input、baseline、live rustdoc snapshot、evaluator contract、および rustdoc-extraction contract の freshness 情報を保持する。completed track の旧 artifact を migration せず、旧値または必要値が使えない場合は保守的に再計算する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D3] [tasks: T005, T011, T013, T018]
- [IN-08] `sotp capability exec` に orchestrator が選ぶ resume option を追加する。同一 track・同一 capability の同一対象成果物への追補、修正再入、中断からの続行だけを対象とし、初回 dispatch と関心事または対象成果物を切り替える dispatch は新規 session とする。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D4] [tasks: T003, T007, T008, T009]
- [IN-09] capability resume cache を、track 内では track・capability・対象 artifact の repo-relative path（複数は正規化済み順序付き集合）、track 外では workspace transient 配下の capability・対象 artifact path で分離する。対象 path が未確定の dispatch は cache を使わず記録もしない。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D4] [tasks: T003, T007, T008]

### Out of Scope
- [OS-01] fast/final の二段 review 構成を廃止または統合すること、または final round を fast の session から resume することは含めない。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D1, knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D2] [tasks: T004]
- [OS-02] repository 外の hosted service で実行される pr-reviewer への model・effort 注入は含めない。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D1] [tasks: T002, T007]
- [OS-05] in-host subagent delegation の resume 対応は含めない。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D4] [tasks: T007]
- [OS-03] session id や freshness state を committed SoT file、review record、track identity に保存すること、または新しい top-level transient path を作ることは含めない。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D2, knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D3, knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D4] [tasks: T003, T005, T007]
- [OS-04] completed track の type-signals artifact を新 schema へ遡及 migration することは含めない。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D3] [tasks: T005, T011, T013, T018]

## Constraints
- [CN-01] 対象 capability の effort が未指定の dispatch を暗黙の provider 既定で実行せず、dispatch を拒否する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D1] [tasks: T001, T002, T007]
- [CN-06] reviewer session の profile または execution-contract fingerprint が不一致、読出し不能、hash 化不能、resume 失敗、または期限切れなら、stale な reuse をせず新規 session で round を実行する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D2] [tasks: T003, T004]
- [CN-07] build-input closure を確定できない場合、または live rustdoc snapshot を検証できない場合は、誤った signal 評価 skip や snapshot reuse をせず、rustdoc 抽出から再計算する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D3] [tasks: T005, T011, T012, T013, T014, T016, T017, T018]
- [CN-08] capability resume cache は、provider・model を含む現在の profile 解決結果、および現在の capability SSoT、dispatcher が常時注入する discipline、capability profile または SSoT が静的 contract input として宣言する policy / contract file の path と内容から得る安定 execution-contract fingerprint に束縛する。profile 解決結果または fingerprint が不一致、読出し不能、hash 化不能、resume 失敗、または期限切れなら、stale な reuse をせず新規 session で dispatch を実行する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D4] [tasks: T003, T007]
- [CN-02] resume 時も dispatcher は現在の profile と provider-native sandbox 定義を通常どおり解決し、model・sandbox・effort を含む全実行 flag を毎回明示的に再注入する。provider の session 設定継承に依存しない。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D2, knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D4] [tasks: T001, T004, T007]
- [CN-03] reviewer は resume 後も scope 全件を再判定する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D2] [tasks: T004, T009]
- [CN-09] session resume は review judgment と review record の単位を変更しない。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D2] [tasks: T004]
- [CN-10] writer 系 capability は resume 後に上流 artifact の変更有無を自ら確認し、変更があれば再読してから作業する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D4] [tasks: T007, T008, T009]
- [CN-04] session cache は gitignored machine-local transient とし、track 解決時は track artifact directory 配下、track 解決不能時は既存 workspace runtime path 配下に置く。cache の lifecycle と key は track または対象 artifact の境界を越えて混線しない。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D2, knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D4] [tasks: T003, T007, T008]
- [CN-05] D3 の reuse 判定は手選別の file list ではなく、対象 rustdoc invocation の完全に解決・正規化された build-input closure と content hash に基づく。判定粒度は layer（crate）単位とし、影響がある layer だけを再計算する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D3] [tasks: T005, T011, T012, T013, T016, T018]

## Acceptance Criteria
- [ ] [AC-01] default routing profile は subprocess を起動する対象 capability の effort を解決でき、reviewer-like capability では fast と final の別 effort が解決できる。default profile の reviewer fast は low、final は各 provider の最大段階であり、fast の前置判定と final の完全判定から成る二段 review を維持する。ref-verifier-chain1 / ref-verifier-chain2 は対象であり、pr-reviewer は対象外である。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D1] [tasks: T001, T002, T004, T007]
- [ ] [AC-02] 対象 capability の effort が欠ける dispatch は provider の既定値で起動せず拒否され、pr-reviewer は effort 未指定でもこの拒否検査の対象にならない。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D1] [tasks: T001, T002, T007]
- [ ] [AC-03] 同一 track・scope・round 種別で fixer 後に再実行した reviewer round は、現在の profile 解決結果と安定 fingerprint が一致する有効な prior session を resume し、reviewer は現在の file list と diff から scope 全件を再判定する。resume は文脈だけを再利用し、その round の judgment と review record は新規 session の round と同じ単位・意味で記録される。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D2] [tasks: T003, T004, T009]
- [ ] [AC-04] reviewer の初回 round、fast から final への escalation、profile または fingerprint 不一致、contract 読出し・hash 化不能、resume 失敗、または session 期限切れでは、新規 session で round を実行する。resume・新規のどちらでも model・sandbox・effort の全 flag は明示指定される。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D2] [tasks: T003, T004]
- [ ] [AC-05] layer の implementation-side input、rustdoc-extraction contract、catalogue declaration、baseline、evaluator contract、および catalogue↔spec 評価に使う spec が対応する記録値とすべて一致し、解決済み live rustdoc JSON を安全に読出し・parse でき、その内容 hash が記録済み snapshot hash と一致するときだけ、reviewer prelude はその signal 評価を skip する。いずれかの評価入力または snapshot 検証が不一致または不明なら、AC-06/AC-07 の条件に従って signal 評価または rustdoc 抽出からの再計算を実行する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D3] [tasks: T015, T005, T011, T012, T013, T014, T016, T017, T018]
- [ ] [AC-06] catalogue、baseline、または catalogue↔spec 評価に使う spec だけが変わった layer は、implementation-side input と rustdoc-extraction contract が一致し、解決済み live rustdoc JSON の安全な読出し・parse と snapshot hash 検証に成功するとき、rustdoc を起動せずに signal 評価を再実行する。evaluator contract の変化は評価を強制するが、この snapshot 条件を満たす限り rustdoc 抽出だけは再利用できる。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D3] [tasks: T015, T005, T011, T012, T013, T014, T016, T017, T018]
- [ ] [AC-07] implementation-side input または rustdoc-extraction contract の不一致・不明、target JSON の不在・読出し・parse 失敗、または snapshot hash 不一致では、snapshot を再利用せず rustdoc 抽出から再計算する。build-input closure を取得または正規化できない場合も同じ経路になる。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D3] [tasks: T005, T011, T012, T013, T014, T016, T017, T018]
- [ ] [AC-08] per-layer type-signals artifact の新 schema は D3 の freshness 情報を計算結果と同じ artifact に記録し、旧 artifact または必要 hash が欠ける artifact を reuse 可能と誤認しない。completed track の artifact は migration しない。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D3] [tasks: T005, T013, T018]
- [ ] [AC-09] orchestrator が同一成果物への継続作業と判断して resume option を指定した provider CLI subprocess dispatch の `sotp capability exec` は、同一 track・capability・正規化済み対象 artifact identity の有効 cache entry だけを再開する。track 解決時は track-local cache、track 解決不能時は workspace-local cache を選び、初回 dispatch、関心事を切り替えた dispatch、対象 path 未確定、contract/profile 不一致、または cache・contract/profile の読出し・hash 化不能な dispatch は、同じ対象 artifact でも新規 session で実行して不適切な cache entry を記録しない。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D4] [tasks: T003, T007, T008]
- [ ] [AC-10] capability resume の再開失敗または session 期限切れは dispatch を中断させず、新規 session に fallback する。resume capability は現在の briefing と上流 artifact を確認し、全実行 flag を再注入したうえで作業する。 [adr: knowledge/adr/2026-07-13-2217-agent-dispatch-cost-reduction.md#D4] [tasks: T003, T007, T008, T009]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Error Handling: Result and ? Operator
- knowledge/conventions/coding-principles.md#No Panics in Library Code
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/responsibility-boundary.md#Rules
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/track-lifecycle.md#Generated Views

## Signal Summary

### Stage 1: Spec Signals
🔵 38  🟡 0  🔴 0

