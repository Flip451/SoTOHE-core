---
adr_id: "2026-07-23-0240-test-obligation-enrollment-mechanization"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session_67282137-412e-4cc0-b687-505dd036e653:2026-07-23"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session_67282137-412e-4cc0-b687-505dd036e653:2026-07-23"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:session_67282137-412e-4cc0-b687-505dd036e653:2026-07-23"
    status: proposed
---
# テスト義務ゲートへの登録を機構化し、成果物不在による空振り合格を廃する

## Context

テスト義務ゲート本体（ADR 2026-07-02-0359）は導出 ⇒ 存在 ⇒ 履行検証の三段で、スコープ内は fail-closed に設計されている。
同 ADR の D15 は check の task-status 連動（todo 帰属の義務は 🟡 許容、in_progress / done は 🔵 必須、strictest-wins）を決定済みで、その効果として「義務導出は Phase 2 直後が最適時点であり、D15 導入後は Phase 2 直後の derive → obligations.json + 空 records の test-bindings.json のコミットが**可能になる**」と明記している（導出の入力＝catalogue / spec / rules は Phase 2 完了時点で確定するため）。

テンプレート実走プロジェクト（mini-repomix）で、実装バッチのコミットを重ねても track ディレクトリに test-obligation の成果物が一切存在しないことが観測された（2026-07-23）。
配線の欠落ではない。
出荷 scaffold の guarded commit は `bin/sotp test-obligation check` を直呼びしており、`test-obligation-rules.json` も出荷されている。

原因は、D15 が開通させた早期導出レーンを「必須」にする工程がどこにも存在しないことである。

1. type-design / impl-plan workflow は test-obligation に一切言及しない。derive の唯一の発動点は implement workflow Step 4 だが、条件は「when applicable — When the track materializes the test-obligation gate」という軟条件で、materialize の判定基準は機械化されていない。完了要件も「once obligation artifacts exist」と成果物の存在に自己参照している。
2. `check` はスコープを成果物の存在で解決し、obligations.json と結び付け表の両方が不在なら 0 件で合格する（check.rs 冒頭に明記された設計）。

帰結として、opt-in しなかった orchestrator を止める機構がなく、D15 の task-status 連動が統制するはずの義務は一度も導出されず、コミットゲートは空振り合格を重ねた。
ゲートはスコープ内では fail-closed だが、スコープへの**登録**が「可能だが任意」のままである。

## Decision

### D1: Phase 2 直後の導出を必須工程にする

type-design workflow の終端に、`bin/sotp test-obligation derive` による `obligations.json` の実体化と、空 records の `test-bindings.json` の実体化（fail-closed codec が受理する明示的な authoring act）を必須ステップとして追加する。
D15 が「可能になる」とした早期導出レーンの工程化であり、両成果物は計画 artifacts と同じコミットに乗る。
導出結果が 0 件でも明示的に実体化し、「不在」と「導出の結果ゼロ」を区別可能にする。
上流（catalogue / spec）への再入時は再導出する。

### D2: check の不在時挙動を「catalogue が存在するなら fail」に改める

`<layer>-types.json` が 1 つでも存在する track で obligations.json（および結び付け表）が不在なら、`check` は fail-closed で落とす。
catalogue を持たない track（フェーズ 0〜1 のコミット、文書のみの track）は現行どおり 0 件で合格する（整合的な不在）。
D15 の task-status 連動は導出済み義務の履行状態を統制する既決であり、本決定はその手前の「導出されていない」状態を閉じる補完である——D1 の workflow に従わない経路（手動運転・workflow 逸脱）もコミットゲートで捕まる二段構えになる。

### D3: 軟条件の語彙を除去し、文書を同期する

implement workflow Step 4 を「binding の増分著作と evaluate ループ」（D15 のバッチごとの増分著作）に限定する記述へ書き換え、「materialize したら」「once obligation artifacts exist」の自己参照条件を除去する。
full-cycle / obligation-fulfillment の前提記述も D1 / D2 の規則に揃える。

## Rejected Alternatives

### A. 現状維持（早期導出は可能だが任意のまま）

実走で穴が実証済み。
スコープ内 fail-closed の設計意図が、登録の任意性によって無効化されている。

### B. implement 開始時の無条件 derive（本 ADR の初稿案）

D15 が最適点とした Phase 2 直後より遅く、計画 artifacts のコミットに義務が同乗しない。
バッチごとの増分著作（D15 の効果）も最初のバッチまで開始できない。

### C. task 状態（in_progress / done の存在）で不在の可否を判定する（本 ADR の初稿案）

検出が実装着手まで遅れ、計画コミット時点の不在を素通しする。
catalogue 存在による判定の方が早く、かつ単純である。

### D. check 側で決定表を再評価し、義務発生源の有無で不在の可否を判定する

決定表（rules）の解釈器が derive と check の 2 箇所に複製され、両者の乖離という新しい発生源を作る。

### E. commit ゲートで毎回 derive を実行する

check は pure-read の設計であり、ゲートが SoT 成果物を書き換えるのは書き込み責務の混入になる。
結び付け表の作成後に暗黙の再導出が走ると、履行関係の churn も招く。

### F. 不在を無条件 fail にする

フェーズ 0〜1 の正当な不在（ADR コミットや仕様のみのコミット）まで落とし、整合的な不在の原則に反する。

## Consequences

### Positive

- 「不在による空振り合格」というクラスが消え、D15 が設計した増分著作レーンが必須工程として機能し始める。
- 義務一覧が計画レビューの時点で実体化・可視化され、計画と義務の突き合わせが可能になる。
- 不在 / 導出ゼロ / 未履行が区別可能になり、track ディレクトリを見ればゲートの登録状態が分かる。

### Negative

- type-design workflow へのステップ追加と、check の不在判定の実装コスト。
- catalogue ありで obligations 不在の既存 track は次のコミットで止まる（それが狙いだが、移行時には derive + 空 records の実体化を一度実行する案内が必要）。

## Reassess When

- catalogue 存在以外の「Phase 2 完了」判定が必要になったとき（catalogue を持たない実装形態の導入など）
- 義務 0 件の空成果物が広く常態化し、ノイズと感じられるようになったとき

## Related

- `knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md` — ゲート本体の正本。特に D15（task-status 連動と Phase 2 直後導出の効果）
- `knowledge/adr/2026-07-11-0802-test-obligation-skipped-status-lane.md`
- `.harness/workflows/track/type-design.md` / `implement.md` / `full-cycle.md` / `obligation-fulfillment.md`
- `libs/usecase/src/test_obligation/check.rs` — 不在時挙動の現行実装
- `.harness/config/test-obligation-rules.json` / `overlay/Makefile.toml` — 出荷済みの決定表とコミットゲート配線
