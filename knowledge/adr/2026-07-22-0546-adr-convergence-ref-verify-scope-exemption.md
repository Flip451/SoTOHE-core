---
adr_id: "2026-07-22-0546-adr-convergence-ref-verify-scope-exemption"
decisions:
  - id: D1
    review_finding_ref: "pr-207-review-round-2-inline-1"
    status: proposed
---
# ADR 収束に対する ref-verify 要求の除外

## Context

フェーズ収束は、参照信号・`bin/sotp ref-verify` による意味論検証・該当 SoT スコープの `zero_findings` review の 3 要素で定義されている。また、spec-design の再開には直上流である ADR の収束（`adr_user` chain）が必要とされている。

`adr_user` chain の ADR 収束に同じ 3 要素を適用すると、ref-verify による意味論検証まで prerequisite として要求される。本 ADR は、`adr_user` chain に対する意味論検証要素の適用要否を明確にする。

## Decision

### D1: `adr_user` chain の収束に意味論検証を要求しない

本決定は `knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md` の D2 と D3 を refine する。

ADR 収束（`adr_user` chain）には、D2 の意味論検証（ref-verify）要素を要求しない。ADR 収束は、次の両方が成立したときに限り成立する。

1. `adr_user` の参照信号が `.harness/config/signal-gates.json` の当該 chain × gate 指定を満たす。
2. ADR scope review が `zero_findings` で完了している。

この除外は `adr_user` chain の ADR 収束だけに限定する。他の chain のフェーズ収束には、D2 の参照信号・意味論検証・レビューという 3 要素を引き続きすべて要求する。

## Rejected Alternatives

### A. ADR 側 scope の有無に応じて意味論検証の要求を切り替える

却下理由: `adr_user` chain では ref-verify を要求しないという規則を、実装されている scope の有無に依存する一時的な例外へ変えてしまう。

### B. すべてのフェーズ収束から意味論検証要素を削除する

却下理由: 本決定の除外対象は `adr_user` chain だけである。他の chain では参照先と引用側の意味的整合を独立に検証する D2 の 3 要素定義を維持する。

### C. 別 chain の既存 scope の通過を ADR 収束の代用証拠とする

却下理由: `adr_user` の意味論を検証しない結果を ADR 側検証の証拠として扱うと、検証対象と収束判定が一致しない。また、D1 は `adr_user` chain に意味論検証を要求しないため、代用証拠を設ける必要がない。

## Consequences

### Positive

- ADR 収束の判定材料が `adr_user` 参照信号と ADR scope review に明確化される
- ref-verify scope の実装状況によって ADR 収束 prerequisite が変動しない
- 他の chain では D2 の 3 要素を維持し、意味論検証を一般に弱めない

### Negative

- ADR 収束には ref-verify による独立した意味論検証証拠を要求しない
- 将来 ADR 側 scope が導入されても、それだけでは ADR 収束 prerequisite に自動追加されない

## Reassess When

- `adr_user` chain に対応する ADR 側の ref-verify scope が導入され、ADR 収束への適用が提案されたとき
- ref-verify の scope と chain の対応関係が変更されたとき
- ADR 収束を検証する別の機械的な意味論検証手段が導入されたとき

## Related

- `knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D2` — D1 が chain ごとの適用範囲を refine するフェーズ収束定義
- `knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D3` — D1 が ADR 収束 prerequisite を refine する再開規則
- `.harness/config/signal-gates.json` — `adr_user` の chain × gate 許容水準の SSoT
