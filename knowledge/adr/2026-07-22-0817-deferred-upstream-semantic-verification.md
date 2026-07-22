---
adr_id: "2026-07-22-0817-deferred-upstream-semantic-verification"
decisions:
  - id: D1
    review_finding_ref: "local-review-harness-policy-final-2026-07-22-0817"
    status: proposed
---
# 上流収束における意味論検証の後段延期

## Context

フェーズ収束は、参照信号・`bin/sotp ref-verify` による意味論検証・該当 SoT スコープの `zero_findings` review の 3 要素で定義され、下流 writer の再開には直上流の収束が要求されている。

一方で、意味論検証 surface が存在する成果物から scope を解決して全 chain の現在の pair を読み込む場合、上流修正で stale になった下流成果物が直後の下流 writer 再実行による再生成を待っていても、その下流 chain の列挙失敗が上流収束に該当する chain の検証を開始前に妨げることがある。

該当 chain だけを独立に検証できない状態で完全な意味論検証を下流 writer の再実行前に要求すると、再生成前の下流成果物が上流収束を妨げ、上流収束の未完了が再生成を担う下流 writer の再実行を妨げる循環が生じる。

## Decision

### D1: 分離不能な上流意味論検証を下流成果物の再生成直後まで延期する

本決定は `knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md` の D2 と D3 を refine する。

意味論検証 surface が上流収束に該当する chain を単独で評価できず、直後に再実行する下流 writer による再生成を待つ成果物の下流 chain まで存在ベースの scope 解決によって評価する場合に限り、上流収束の意味論検証要素をその下流成果物の再生成直後まで延期する。

延期中であっても、上流の参照信号が `.harness/config/signal-gates.json` の当該 chain × gate 指定を満たすことと、上流の該当 SoT スコープ review が `zero_findings` で完了していることは、下流 writer の再 dispatch 前に必須とする。

下流 writer の dispatch briefing は、延期した意味論検証、対象となる上流 chain、および成果物再生成後に必要な full `bin/sotp ref-verify run` を明記しなければならない。

下流 writer が成果物を再生成した直後、さらに下流のフェーズへ進む前に full `bin/sotp ref-verify run` を実行し、延期した上流 chain を含む全対象の通過によって意味論検証要素を充足する。

再生成後の full verification が失敗した場合は延期を継続したまま先へ進まず、`knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md` の D4 に従って失敗を所有する上流へ即時に突き返す。

この延期は検証時点の移動であり、意味論検証の免除または失敗の許容ではない。

## Rejected Alternatives

### A. full verification の失敗後に保存済み結果から上流 chain の通過だけを判定する

却下理由: 下流 chain の列挙失敗が上流 chain の評価開始自体を妨げる場合、上流の fresh な結果は存在せず、保存済み結果では修正後の上流収束を証明できない。

### B. 上流の参照信号と scope review を満たさないまま下流 writer を先行実行する

却下理由: 上流の参照信号と scope review まで省略する無制約な先行実行は、順次処理規律が要求する再開条件を必要以上に弱める。D1 が許容するのは両要素を満たした後に意味論検証要素だけを延期する制約付きの再 dispatch であり、成果物再生成直後の mandatory verification は免除しない。

### C. 分離不能な chain の意味論検証を収束要素から除外する

却下理由: 検証不能を免除へ変えると上流と再生成後の下流成果物の意味的整合を確認しないまま降下できるため、再生成直後の mandatory verification と即時突き返しによって検証義務を維持する。

## Consequences

### Positive

- stale な下流成果物の再生成と上流 chain の意味論検証が互いを待つ循環を解消できる
- 参照信号と scope review の事前要件を維持し、延期対象を意味論検証要素だけに限定できる
- 再生成直後の full verification と失敗時の即時突き返しにより、意味論検証を waiver にせず必須のまま維持できる

### Negative

- 下流 writer の briefing と再生成後の検証時点で延期状態を明示的に引き継ぐ必要がある
- 上流フェーズの再 dispatch 時点では、3 要素のうち意味論検証だけが後段で充足される一時状態を扱う必要がある
- full verification が失敗した場合、再生成した下流成果物を保持したまま上流へ即時に戻る追加往復が発生する

## Reassess When

- `bin/sotp ref-verify` に chain を単独で評価できる surface が導入されたときは、本延期を廃止して下流 writer の再 dispatch 前に意味論検証を完了する
- 存在ベースの scope 解決が、再生成待ちの下流成果物によって上流 chain の評価を妨げない方式へ変更されたとき
- full verification の対象選択または結果保存方式が変わり、上流 chain の fresh な通過を下流成果物の再生成前に証明できるようになったとき

## Related

- `knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D2` — D1 が意味論検証要素の充足時点を限定的に refine するフェーズ収束定義
- `knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D3` — D1 が下流 writer の再開 prerequisite を限定的に refine する直上流検査規則
- `knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md#D4` — 再生成後の full verification 失敗を即時に上流へ突き返す規則
