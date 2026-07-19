---
adr_id: 2026-07-19-0333-escalation-reason-verdict-provenance
decisions:
  - id: D1
    review_finding_ref: "github_pr:#203:round-1:external-reviewer:2026-07-19 primary ADR の expected Phase 1+ escalation には guardian verdict が存在せず、既存 D3 の一律必須要件と両立しないとの指摘"
    candidate_selection: "from:[update-or-supersede-requirement,retain-always-applicable-provenance-field] chose:update-or-supersede-requirement"
    status: proposed
---
# escalation reason の guardian verdict 来歴を判定の存在時に限定する

## Context

2026-07-19 の PR #203 round 1 review で、primary ADR の escalation reason に guardian verdict の要旨を一律に要求する `2026-07-18-0340-adr2pr-baseline-diff-comment.md` D3 と、`2026-07-16-2001-adr-decision-freeze.md` D6 が定める expected Phase 1+ escalation の直接刻印レーンが両立しないことが判明した。後者は予期された編集であるため adr-diagnoser を経由せず、記録すべき guardian verdict 自体が存在しない。

## Decision

### D1: guardian verdict の要旨は判定が存在するときだけ escalation reason に要求する

primary ADR の escalation reason は、常に起点入力の由来と要旨を自己完結に記録する。起点入力は少なくとも local review round、外部 PR review round、spec→ADR の 🔴 signal（該当要素と参照）、および diagnose routing（診断入力と ADR へ戻した理由）を区別できなければならない。そのうえで、Phase 0 の裁定を経た刻印など guardian verdict が存在する場合に限り、その判定結果の要旨も記録する。

expected Phase 1+ escalation の直接刻印では、起点入力の来歴に加えて、「これは予期された escalation であり、adr-diagnoser を経由しないため guardian verdict は存在しない」と reason 自体に明記する。終端の Step 11 renderer はこの記録の guardian verdict 欄を `該当なし` と表示し、欠落または不十分な来歴として扱わない。

本 D1 は `2026-07-18-0340-adr2pr-baseline-diff-comment.md` D3 を置き換え、guardian verdict 要旨は判定が存在する場合だけ必須とする。D3 の起点入力の来歴要件、および存在する guardian verdict の要旨を永続化する要件は維持する。

## Rejected Alternatives

- **常に適用できる別の provenance field を追加する**: expected escalation に存在しない guardian verdict の代替値を持たせると、起点入力と編集判定という異なる来歴を曖昧にする。判定の非存在を明示し、renderer が `該当なし` と表現する方が監査上正確である。

## Consequences

### Positive

- guardian verdict が存在する escalation は従来どおり判定要旨を保持し、expected escalation は起点入力と判定非存在の理由から自己完結した来歴を保持できる。
- Step 11 renderer は `該当なし` を正規の状態として表示できる。

### Negative

- reason の検証と renderer は、guardian verdict の有無を escalation lane に応じて解釈する必要がある。

## Reassess When

- expected escalation にも guardian verdict を生成・永続化する経路が導入されたとき。
- escalation reason が自由記述から構造化 schema へ移行したとき。

## Related

- `2026-07-18-0340-adr2pr-baseline-diff-comment.md#D3` — 本 D1 が guardian verdict 要旨の適用条件を精緻化して置き換える対象
- `2026-07-16-2001-adr-decision-freeze.md#D6` — expected Phase 1+ escalation を diagnoser なしで直接刻印する決定
