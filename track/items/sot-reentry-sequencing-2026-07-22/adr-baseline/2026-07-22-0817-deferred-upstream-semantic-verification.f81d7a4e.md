---
adr_id: "2026-07-22-0817-deferred-upstream-semantic-verification"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session_01ESUACDZiuzbJG2RrG83Foa:2026-07-22-delta-adoption"
    status: proposed
---
# 上流収束における意味論検証の chain scope 明確化

## Context

フェーズ収束は、参照信号・`bin/sotp ref-verify` の該当 scope による意味論検証・該当 SoT スコープの `zero_findings` review の 3 要素で定義されている。

`bin/sotp ref-verify run` は現在存在する成果物から全 chain の pair を列挙するため、上流修正で stale になった下流成果物が存在しない spec anchor を参照すると、その下流 chain の列挙失敗が上流 chain の評価前に run 全体を中断し、その run では cache が更新されない。

一方で、`bin/sotp ref-verify results --chain 1` のような chain 限定の読み出しは他 chain の整合性検査を経ず、永続化済み cache から当該 chain の結果を独立に取得できる。

上流収束に full run 全体の通過を要求すると、D2 の「該当 scope」を当該上流 chain 以外へ広げ、直後の下流 writer 再実行で再生成される成果物に起因する別 chain の失敗まで上流収束の条件として扱ってしまう。

## Decision

### D1: 上流収束の意味論検証を当該上流 chain の指摘解消に限定する

本決定は `knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md` の D2 にある「該当 scope」の解釈を refine する。

上流収束の意味論検証要素は、当該上流 chain に関係する `ref-verify` 指摘がすべて解消されていることだけを問う。

他 chain の指摘、および直後の下流 writer 再実行で再生成される stale な下流成果物に起因する pair 列挙失敗は、当該上流 chain の指摘ではないため上流収束の判定に関与しない。

既知の当該 chain 指摘が解消されていることの確認には、`bin/sotp ref-verify results --chain 1` のような chain 限定の読み出しを用いる。

他 chain の列挙失敗によって fresh な検証結果を生成できないこと自体は、当該上流 chain の指摘が未解消であることを意味せず、意味論検証要素を未充足にしない。

`bin/sotp ref-verify run` は pair を列挙可能になり次第実行し、通常は上流修正後に即時実行し、他 chain の列挙失敗で中断した場合は下流 writer が成果物を再生成した直後に full run を実行する。

後続の run で当該上流 chain の指摘が生じた場合は、`knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md` の D4 に従ってその指摘を所有する上流へ即時に突き返す。

上流の参照信号が `.harness/config/signal-gates.json` の当該 chain × gate 指定を満たすことと、上流の該当 SoT スコープ review が `zero_findings` で完了していることは、従来どおり下流 writer の dispatch 前に必須とする。

本決定は意味論検証要素の判定対象を chain 単位で明確化するものであり、フェーズ収束の充足時点を変更せず、当該上流 chain の指摘を免除しない。

## Rejected Alternatives

### A. 上流収束に full `bin/sotp ref-verify run` 全体の通過を要求する

却下理由: D2 の「該当 scope」を当該上流 chain 以外へ広げ、別 chain の指摘や再生成待ちの下流成果物に起因する列挙失敗まで上流収束の条件へ混入させる。

### B. pair 列挙が可能になるまで上流収束の意味論検証要素を未充足として扱う

却下理由: 当該上流 chain の指摘ではない下流 chain の列挙失敗を意味論検証要素の未充足へ読み替え、検証時点を下流成果物の再生成後へ移すことで D2 の chain scope を timing 変更として扱ってしまう。

### C. 列挙失敗後は当該上流 chain の検証を再実行しない

却下理由: 列挙失敗が当該 chain の未解消を意味しないことと、列挙可能になった後の検証義務は別であり、後続 run で生じた当該 chain の指摘には D4 の即時突き返しを適用しなければならない。

## Consequences

### Positive

- 上流収束が当該上流 chain の指摘解消だけを問い、別 chain の状態を誤って prerequisite に含めない
- 既存の chain 限定 results 読み出しを用いて、full run の全体 status と当該上流 chain の状態を分離できる
- 参照信号と scope review の dispatch 前要件、および後続 run で発見された当該 chain 指摘への即時突き返しを維持できる

### Negative

- full run の失敗だけでは上流収束を判定できず、失敗した chain と chain 限定 results を区別して解釈する必要がある
- 列挙失敗中は最新変更に対する fresh な当該 chain 結果を生成できず、列挙可能になった直後の run が新しい指摘を発見する可能性がある
- 後続 run で当該上流 chain の指摘が見つかった場合、下流成果物の再生成後でも D4 に従って上流へ戻る必要がある

## Reassess When

- `bin/sotp ref-verify run` に chain 単位の実行 selector が導入され、他 chain の列挙失敗から独立して fresh な当該 chain 結果を生成できるようになったとき
- `bin/sotp ref-verify results --chain` の cache 読み出しまたは chain 分離の意味が変わったとき
- D2 の「該当 scope」と chain の対応関係が変更されたとき

## Related

- `knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D2` — D1 が意味論検証要素の「該当 scope」を当該上流 chain の指摘に限定して refine するフェーズ収束定義
- `knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D4` — 後続 run で当該上流 chain の指摘が生じた場合に適用する即時突き返し規則
