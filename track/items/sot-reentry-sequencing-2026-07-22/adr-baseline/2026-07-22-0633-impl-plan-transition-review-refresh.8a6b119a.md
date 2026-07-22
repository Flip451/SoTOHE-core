---
adr_id: "2026-07-22-0633-impl-plan-transition-review-refresh"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session_01ESUACDZiuzbJG2RrG83Foa:2026-07-22-delta-adoption"
    status: proposed
---
# impl-plan task ステータス遷移後の review refresh

## Context

`impl-plan.json` の review 収束後に許容される task ステータス遷移は、SoT 再入の順次処理において収束を失効させない例外として定められている。

一方で、task ステータス遷移は `impl-plan.json` の内容と hash を変更するため、遷移前の review 証跡をそのまま commit gate に用いることはできない。

## Decision

### D1: task ステータス遷移例外を再入 sequencing に限定する

本決定は `knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md` の D5 を refine する。

review 収束後の `impl-plan.json` に許容する変更種別は、従来どおり `bin/sotp track transition` による task ステータス遷移だけとする。

この変更が「収束を失効させない」とは、SoT 再入の sequencing 規律において上流フェーズへの rollback と下流作業の停止を要求しないことだけを意味する。

task ステータス遷移によって `impl-plan.json` の hash が変わった場合、`.harness/workflows/track/full-cycle.md` の lifecycle tail に従い、commit 前に current hash を対象とする impl-plan scope の final `zero_findings` review を改めて完了しなければならない。

D5 の例外は、この hash-based commit gate の review refresh を免除しない。

## Rejected Alternatives

### A. task ステータス遷移後も遷移前の review 証跡を有効とみなす

却下理由: review 証跡が対象とする hash と commit 対象の `impl-plan.json` が一致せず、hash-based commit gate の前提を満たさない。

### B. review refresh が必要な変更をすべて D5 の許容対象へ広げる

却下理由: D5 が許容する変更種別は task ステータス遷移だけであり、他の `impl-plan.json` 変更には上流 rollback と下流停止を伴う再収束が必要である。

### C. task ステータス遷移を D4 と同じ収束失効として扱う

却下理由: commit 前の review refresh は必要だが、task ステータス遷移のたびに上流フェーズへ rollback して下流作業を停止する必要はない。

## Consequences

### Positive

- D5 の例外が再入 sequencing と review 証跡の freshness を混同しない
- task ステータス遷移では上流 rollback と下流停止を避けながら、commit gate に current hash の review 証跡を提示できる
- D5 の許容変更種別を task ステータス遷移だけに維持できる

### Negative

- final task ステータス遷移後に impl-plan scope の review を再実行する必要がある
- sequencing 上の収束と commit gate 用 review 証跡の有効性を別々に追跡する必要がある

## Reassess When

- task ステータス遷移が `impl-plan.json` の hash を変更しない機構へ移行したとき
- commit gate が review 証跡を hash 以外の方法で対象成果物へ束縛するようになったとき
- `.harness/workflows/track/full-cycle.md` の lifecycle tail における final review の要件が変更されたとき

## Related

- `knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D5` — D1 が再入 sequencing に限定して refine する task ステータス遷移例外
- `.harness/workflows/track/full-cycle.md` — task ステータス遷移後に current hash の final `zero_findings` review を要求する lifecycle tail
