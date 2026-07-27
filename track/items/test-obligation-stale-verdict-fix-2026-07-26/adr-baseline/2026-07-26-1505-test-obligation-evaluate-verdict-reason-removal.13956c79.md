---
adr_id: 2026-07-26-1505-test-obligation-evaluate-verdict-reason-removal
decisions:
  - id: D1
    user_decision_ref: "chat_segment:current-task:2026-07-28-delta-adoption"
    status: proposed
---
# `test-obligation evaluate` の失敗結果から非正本の理由フィールドを削除する

## Context

`sotp test-obligation evaluate` の失敗結果では、各 entry の実際の判定理由が `entries[].verdict.reason` に存在する一方、同じ entry の top-level にある `entries[].verdict_reason` は常に null だった。この null を「理由が記録されていない」と解釈した誤ったエスカレーションが実際に発生しており、非正本フィールドの存在そのものが診断を誤らせている。

既存の D6 は fail 類型と裏付け引用、D11 は `evaluate` の CLI surface と fail-closed 挙動、D14 は `results` の informational 性質を定めるが、`evaluate` の失敗結果におけるデータ形状と理由の格納先は定めていない。したがって、常に null のフィールドを互換性だけのために残す根拠は既存決定から導出できない。

また、spec → ADR の commit gate は strict であり、user grounds に置いた要素は Yellow に留まる。convention 参照はこの signal 評価に含まれないため、この欠落を下流の grounds 変更だけで決定保存的に解消する経路はなく、失敗結果スキーマの契約を ADR として記録する必要がある。

## Decision

### D1: `entries[].verdict_reason` を削除し、失敗理由の正本を `entries[].verdict.reason` に限定する

`sotp test-obligation evaluate` の結果 envelope に含まれる各 entry から、top-level の `entries[].verdict_reason` を削除する。出力 schema、domain model、encoder はこのフィールドを表現または出力せず、null placeholder、alias、mirror のいずれも設けない。

失敗 verdict の実際の判定理由は `entries[].verdict.reason` のみを正本とし、失敗 verdict ではこの値を必須かつ非空にする。caller は失敗理由の取得と理由記録の有無の判定にこの nested field だけを使い、同じ意味を持つ sibling field の存在を前提にしない。

本決定は、D6、D11、D14 が規定していない外部観測可能な `evaluate` の失敗結果スキーマを新規に定める独立決定であり、supersede または refine の対象を持たない。D6 の fail 類型と裏付け引用、D11 の CLI surface と fail-closed 挙動、D14 の `results` が informational であるという責務分離は変更しない。

## Rejected Alternatives

### A. `entries[].verdict_reason` を常に null の互換フィールドとして残す

却下する。既存 decoder がこのフィールドを必要とする実在根拠がなく、新しい caller にも情報を与えない。互換性だけのために非正本フィールドを維持することは、不要な compatibility layer を最小化する方針に反するうえ、理由が欠落したという誤読を残す。

### B. `entries[].verdict.reason` を `entries[].verdict_reason` にミラーする

却下する。同じ理由を二箇所に複製すると、値の不一致という新たな不正状態を表現可能にし、どちらが正本かという曖昧さを再導入する。

### C. 現行形状を決定せず、caller の読み分けに委ねる

却下する。常に null の sibling field を誤読する経路が残り、失敗理由の取得方法を caller ごとの暗黙知にしてしまう。

## Consequences

### Positive

- 失敗理由の正本が `entries[].verdict.reason` の一箇所になり、null の sibling field による誤診断を排除できる。
- 不要な compatibility field と二重表現を持たないため、producer と caller の解釈が一致する。
- 失敗 verdict で非空理由を要求することで、fail-closed の診断結果に理由がない状態を schema 上で拒否できる。

### Negative

- `entries[].verdict_reason` の存在を要求する decoder は更新が必要になり、旧 decoder との wire 互換性は維持しない。
- active scope では schema 変更を即時適用するため、producer と caller を同じ変更単位で更新する必要がある。

### Active Scope and Legacy Behavior

本決定は、この schema 変更を含む版の `sotp test-obligation evaluate` を使用して作業する active track に即時適用し、grace period、opt-out、旧 field の読み書き fallback を設けない。

completed、merged、archive 済みの non-active track と、それらに残る旧 schema の結果・記録は遡及的に再評価、再生成、書き換えしない。旧版が出力済みの `entries[].verdict_reason: null` はその旧 schema の履歴として残り得るが、新しい producer は出力せず、新しい schema は旧 field の受理を互換契約として保証しない。

## Reassess When

- 失敗理由を複数件または構造化された reason code として表現する必要が生じ、単一の `entries[].verdict.reason` では診断情報を保持できなくなったとき。
- `evaluate` の結果 envelope 全体に明示的な schema versioning と version 間変換を導入する別の決定が採用されたとき。

## Related

- `knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D6` — fail 類型と判定理由の意味論を定める既存決定。本決定は類型と裏付け引用の規律を変更しない。
- `knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D11` — `evaluate` の CLI surface と fail-closed 挙動を定める既存決定。失敗結果スキーマは D11 の規定領域に含まれず、本決定が独立して定める。
- `knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D14` — `results` の informational 性質を定める既存決定。本決定は `evaluate` の出力だけを扱い、`results` の責務を変更しない。
- `knowledge/conventions/no-backward-compat.md` — schema 変更の active scope、legacy 挙動、遡及非適用を定める規約。
