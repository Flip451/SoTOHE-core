---
adr_id: "2026-08-25-2239-required-ports-exempt-from-necessity-test"
decisions:
  - id: D1
    review_finding_ref: "pr_review:253:5019497894:type-designer-kind-selection:required-ports-exempt"
    status: proposed
---
# 必須ポートを必要性テストの対象外にする

## Context

`2026-08-20-1043-conventions-mechanism-alignment.md` D2 は、共有所有だけを理由にした抽象を避けるため、抽象の導入を複数実装またはテスト境界での差し替えが必要な場合に限定した。この表現をすべての port trait に適用すると、ユースケースごとに一つの入力ポートを置く構造規則と、層を越える依存を trait 境界に置くヘキサゴナル原則まで禁止することになる。

入力ポートと secondary port は、任意の service を `Arc<dyn>` で共有するための抽象ではなく、アーキテクチャが要求する境界そのものである。必要性テストの対象と区別しなければ、具体サービスまたは infrastructure adapter を層境界を越えて直接参照する設計を許す余地が生じる。

## Decision

### D1: 必須ポートは必要性テストの対象外とする

`2026-08-20-1043-conventions-mechanism-alignment.md` D2 の必要性テストは、役割名ではなく導入理由で適用範囲を決める。対象は、構造規則が要求する port とは別に、層の内部で任意に追加する trait と実装の組である。既存の具体 service の上に、`Arc<dyn>` で共有するためだけにこの組を重ねる場合、および複数実装もテスト境界での差し替えもないまま同様の抽象を追加する場合は、必要性テストの対象とする。

次の port trait は必要性テストの対象外とし、複数実装またはテスト差し替えの有無で導入可否を判断しない。

- inbound port trait: ユースケースごとに一つの実行メソッドを持つ、そのユースケース自身の入力ポート。これは `ApplicationService` 役割の trait であっても構造規則が要求するものであり、必要性テストの対象にしない。
- secondary port: 層を越える依存を表す出力側のポート。

この二つは、必要性テストではなく支配するアーキテクチャ規則に従って導入する structure-required ports である。ユースケース自身の入力ポートの上に、同じ service を共有するためだけの第二の trait と実装を追加する場合は、この対象外には含めず、必要性テストを適用する。

## Rejected Alternatives

- **すべての port trait に必要性テストを適用する**: 必須の入力境界と層間依存境界まで、導入理由を区別せず任意の抽象として扱い、構造規則と矛盾する。
- **入力ポートだけを対象外にする**: secondary port を具体 infrastructure adapter への直接依存へ置き換える余地が残り、ヘキサゴナル境界を保てない。
- **secondary port だけを対象外にする**: ユースケースごとの入力ポートという構造規則を必要性テストで妨げることになる。

## Consequences

- Good: 必須の入力境界と層間依存境界を維持したまま、任意の service-level 抽象の増殖を防げる。
- Good: `ApplicationService` という役割名だけで判定せず、構造規則が要求する inbound port と任意に追加する抽象を区別できる。
- Bad: type designer は port の種類を先に分類し、必要性テストの適用対象かを判断する必要がある。

## Reassess When

- inbound port または secondary port 以外に、アーキテクチャ規則によって trait 導入が構造的に必須となる port の種類が追加されたとき。

## Related

- `knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md` D2 — 本決定が refine する現在の関係連鎖の対象。必要性テストの適用先を任意の service-level 抽象に限定する。
- `knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md` D3 — inbound port のユースケース単位の粒度を定める。
- `knowledge/adr/2026-08-15-1302-composition-root-pure-di-port-granularity.md` D1 — inbound port が一ユースケース一 trait・一実行メソッドである構造規則を定める。
- `knowledge/conventions/type-designer-kind-selection.md` R1 — `ApplicationService` と `SecondaryPort` の役割および層配置を定める。secondary port はこの配置規則とヘキサゴナル原則により導入する。
