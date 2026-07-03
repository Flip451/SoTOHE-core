<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 19, yellow: 0, red: 0 }
---

# レビュー負荷軽減 — findings 全件報告と下流 artifact の再記述禁止

## Goal

- [GO-01] reviewer の findings 全件報告規律（1 round で severity policy に該当する findings を全件列挙して報告する）と、下流 artifact（impl-plan の task text / plan sections、型カタログの docs / intent）が上流（ADR / spec）の設計理由・挙動契約を散文で再記述しないことを求める convention の新設、およびその convention を reviewer severity policy 更新（実行可能性の再定義 + 再記述の finding 化）とセットで review gate に載せることにより、findings 1 件ずつの直列往復と artifact 間矛盾 findings の発生源を減らし、レビュー負荷を軽減する。 [adr: knowledge/adr/2026-07-02-1600-review-load-batch-findings-no-restatement.md#D1, knowledge/adr/2026-07-02-1600-review-load-batch-findings-no-restatement.md#D2, knowledge/adr/2026-07-02-1600-review-load-batch-findings-no-restatement.md#D3]

## Scope

### In Scope
- [IN-01] `.harness/config/review-scope.json` の `groups[].briefing_file` が参照する `.harness/custom/review-prompts/` 配下の review prompt ファイルすべて（`spec.md` / `types.md` / `impl-plan.md` / `domain.md` / `usecase.md` / `infrastructure.md` / `cli.md` / `cli_composition.md` / `cli_driver.md` / `harness-policy.md` / `adr.md`）に、「severity policy に該当する findings は、その round で発見した全件を列挙して報告する」規律を明記する。既存の severity 基準（事実誤り・矛盾・実行不能・broken reference のみを報告する既存の基準）の文言そのものは変更しない。 [adr: knowledge/adr/2026-07-02-1600-review-load-batch-findings-no-restatement.md#D1] [tasks: T001]
- [IN-02] `knowledge/conventions/` 配下に新規 convention ファイルを追加し、impl-plan の task text / plan sections と型カタログ（`<layer>-types.json`）の docs / intent は「変更対象（file / symbol）+ 操作 + spec anchor の cite」で記述し、上流（ADR / spec）の設計理由・挙動契約を散文で再説明しないと定める。挙動は `AC-NN` / `IN-NN` / `CN-NN` の cite で参照し内容を言い直さないこと、数値状態（`schema_version` 等）は literal ではなく相対参照（「現行値 + 1」等）で書くことをルールに含める。追加後は `knowledge/conventions/README.md` の Current Files 一覧にも登録する。 [adr: knowledge/adr/2026-07-02-1600-review-load-batch-findings-no-restatement.md#D2] [tasks: T002]
- [IN-03] `.harness/custom/review-prompts/impl-plan.md` の severity policy にある「task description non-executable」の判定基準を、「変更対象 file / symbol + 操作 + anchor cite が揃っていれば実行可能」に書き換える。挙動の再説明（現行の『what the expected behaviour is』の要求）を実行可能性の要件から外す。 [adr: knowledge/adr/2026-07-02-1600-review-load-batch-findings-no-restatement.md#D3] [tasks: T003]
- [IN-04] `.harness/custom/review-prompts/impl-plan.md` と `.harness/custom/review-prompts/types.md` の両方の severity policy に、「上流（ADR / spec）の挙動・設計理由を散文で再説明している」ことを finding class として新設する。reviewer の判断軸を、散文同士の意味調停（どちらが正しいかの調査）から、再記述の存在検出 + citation の妥当性確認へ寄せる記述にする。 [adr: knowledge/adr/2026-07-02-1600-review-load-batch-findings-no-restatement.md#D3] [tasks: T003, T004]

### Out of Scope
- [OS-01] spec.json は再記述禁止 convention（IN-02）の対象外とする。ADR を細粒度化して挙動契約に落とし込むのが spec.json の本務であり、上流内容の書き下しはその本質的な役割であるため、再記述禁止のルールを適用しない。 [adr: knowledge/adr/2026-07-02-1600-review-load-batch-findings-no-restatement.md#D2]
- [OS-02] 本 track 開始前に作成済みの既存 track artifact（impl-plan.json / `<layer>-types.json` 等）に対する、再記述禁止 convention（IN-02）の遡及的な書き直しは行わない。完了済み track の artifact は歴史的記録として原型を保つ。 [adr: knowledge/adr/2026-07-02-1600-review-load-batch-findings-no-restatement.md#D2]
- [OS-03] workflow ドキュメント（`.claude/commands/` / `.claude/skills/` 等）は再記述禁止 convention（IN-02）の適用対象に含めない。provider 非依存 logic の重複禁止という既存の adapter-SSoT 規則が同族の懸念として既にカバーしている。 [adr: knowledge/adr/2026-07-02-1600-review-load-batch-findings-no-restatement.md#D2]
- [OS-04] fast → final の 2 round review 構造は変更しない（省略・確率化は行わない）。clean な fast round と同一 hash に対する final round が実測で 72 件中 26 件（36%）で実 findings を捕捉しており、省略は品質を直撃するため対象外とする。D1 が変更するのは 1 round あたりの findings 報告密度のみであり、round 構造そのものは D1 のスコープ外として維持する。 [adr: knowledge/adr/2026-07-02-1600-review-load-batch-findings-no-restatement.md#D1]
- [OS-05] per-scope hash 隔離の強化・同一 hash 再実行時の verdict cache 機構の新設は行わない。hash は既にスコープの対象 file 集合限定の manifest で計算され隔離済みであり、同一 hash の重複再実行も実測で 205 round 中 5 回に留まるため、単独対策としては見合わない。D1 が対象とする round 内の報告密度とは独立した round 実行機構の変更であり、D1 のスコープ外として維持する。 [adr: knowledge/adr/2026-07-02-1600-review-load-batch-findings-no-restatement.md#D1]
- [OS-06] 散文中の anchor token（`AC-NN` / `CN-NN` / `D<n>` 等）の実在検査・レイヤー適合検査を機械 lint として実装することは対象外とする。引用・例示・打ち消し文脈での false positive の扱いが難しく、今回は D3 が定める reviewer の意味論判断（IN-04 の finding class 追加）で代替する。 [adr: knowledge/adr/2026-07-02-1600-review-load-batch-findings-no-restatement.md#D3]
- [OS-07] sotp（Rust CLI）本体のコード変更は本 track の対象外とする。実装は `.harness/custom/review-prompts/*.md` と `knowledge/conventions/` への文書変更のみで完結する（D1 / D2 / D3 のいずれも prompt / convention の文書変更のみで実現する）。 [adr: knowledge/adr/2026-07-02-1600-review-load-batch-findings-no-restatement.md#D1, knowledge/adr/2026-07-02-1600-review-load-batch-findings-no-restatement.md#D2, knowledge/adr/2026-07-02-1600-review-load-batch-findings-no-restatement.md#D3]

## Constraints
- [CN-01] 全件報告規律（IN-01）の追記は、既存の severity 制約（事実誤り・矛盾・実行不能・broken reference のみを報告する基準）を緩めない。変更するのは『該当する findings を全件列挙する』という報告密度の規律のみであり、報告してよい findings の種類・基準文言自体は変更しない。 [adr: knowledge/adr/2026-07-02-1600-review-load-batch-findings-no-restatement.md#D1] [tasks: T001]
- [CN-02] D2（再記述禁止 convention の新設）と D3（reviewer severity policy の実行可能性再定義 + 再記述の finding 化）は同一 track 内でセットとして提供する。convention 文書の新設のみを先行させ、reviewer severity policy 更新を後続 track に切り出すことはしない。gate 強制のない文書ルール単独は形骸化する（実測: gate 強制なし文書ルール = 25 違反、gate 強制あり = 0 違反）ことが、convention 文書のみで運用する代替案を却下した理由である。 [adr: knowledge/adr/2026-07-02-1600-review-load-batch-findings-no-restatement.md#D3] [tasks: T002, T003, T004]

## Acceptance Criteria
- [ ] [AC-01] `.harness/config/review-scope.json` の `groups[].briefing_file` が参照する review prompt ファイルそれぞれに、severity policy に該当する findings をその round で発見した全件列挙して報告する旨の指示が明記されている。既存の severity 基準（事実誤り・矛盾・実行不能・broken reference）の記述内容が変更前と同一であることを diff で確認できる。 [adr: knowledge/adr/2026-07-02-1600-review-load-batch-findings-no-restatement.md#D1] [tasks: T001]
- [ ] [AC-02] `knowledge/conventions/` 配下に、下流 artifact（impl-plan の task text / plan sections、型カタログの docs / intent）の再記述禁止ルールを定めた convention ファイルが存在する。本文に、挙動を `AC-NN` / `IN-NN` / `CN-NN` の cite で参照し内容を言い直さないこと、数値状態を相対参照で書くこと、spec.json は対象外であること、既存 track artifact への遡及適用を行わないことが明記されている。`knowledge/conventions/README.md` の Current Files 一覧に新規ファイルが登録されている。 [adr: knowledge/adr/2026-07-02-1600-review-load-batch-findings-no-restatement.md#D2] [tasks: T002]
- [ ] [AC-03] `.harness/custom/review-prompts/impl-plan.md` の「task description non-executable」の判定基準が「変更対象 file / symbol + 操作 + anchor cite が揃っていれば実行可能」に書き換えられており、挙動の再説明を求める記述（『what the expected behaviour is』）が実行可能性の要件として残っていない。 [adr: knowledge/adr/2026-07-02-1600-review-load-batch-findings-no-restatement.md#D3] [tasks: T003]
- [ ] [AC-04] `.harness/custom/review-prompts/impl-plan.md` と `.harness/custom/review-prompts/types.md` の両方に、「上流（ADR / spec）の挙動・設計理由を散文で再説明している」ことを finding class として報告する項目が新設されている。 [adr: knowledge/adr/2026-07-02-1600-review-load-batch-findings-no-restatement.md#D3] [tasks: T003, T004]
- [ ] [AC-05] 本 track の commit 履歴において、D2（convention 新設）と D3（reviewer severity policy 更新）の変更が同一 track 内で提供されており、convention 文書のみを先行 commit して reviewer policy 更新を後続 track に分離していないことを確認できる。 [adr: knowledge/adr/2026-07-02-1600-review-load-batch-findings-no-restatement.md#D3] [tasks: T002, T003, T004]

## Related Conventions (Required Reading)
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/adr.md#ADR vs Convention

## Signal Summary

### Stage 1: Spec Signals
🔵 19  🟡 0  🔴 0

