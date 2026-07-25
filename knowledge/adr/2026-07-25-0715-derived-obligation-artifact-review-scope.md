---
adr_id: "2026-07-25-0715-derived-obligation-artifact-review-scope"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session_01Nrdjbv6vha9cznfwcGDEo6:2026-07-25:obligations-review-operational"
    status: proposed
---
# 機械導出される義務成果物を review 運用成果物として扱う

## Context

`track/items/<track-id>/obligations.json` は `bin/sotp test-obligation derive` が型カタログから機械的に導出する成果物である。
`.harness/workflows/track/type-design.md` の終端ステップが導出し、spec / カタログの再生成を伴う back-and-forth 再入のたびに再導出される。
内容は導出元の関数であり、人手の判断を含まない。

`.harness/config/review-scope.json` の `review_operational` には、同じ性質を持つ per-track 派生物が既に列挙されている。
`*-type-signals.json`、`*-catalogue-spec-signals.json`、各種 verify-cache、`obligation-fulfillment-cache.json`、`adr-baseline/**` がそれにあたる。
これらは scope 分類の対象外となり、再生成が scope hash を動かさない。

`obligations.json` はどの group pattern にも一致せず、`other` scope に落ちている。
その結果、再導出のたびに `other` scope の hash が動き、承認済みのレビューが失効する。
再導出の中身は導出元から決まるため、失効して再実行されるレビューが新たに判定する対象は存在しない。

同じ track が導出する `test-bindings.json` は性質が異なる。
binding 記録は実装フェーズで書き手が明示的に authoring するものであり、機械導出ではない。

## Decision

### D1: `obligations.json` を `review_operational` に加える

`.harness/config/review-scope.json` の `review_operational` に `track/items/<track-id>/obligations.json` を加え、scope 分類の対象外とする。

判定基準は成果物の重要度ではなく、その内容が機械導出であるかに置く。
導出元がレビュー対象である限り、導出結果を独立にレビューしても新たに判定する対象は生じない。

`test-bindings.json` はこの決定の対象外とし、レビュー対象のまま残す。
binding 記録は書き手の明示的な authoring であり、導出関数の出力ではない。

## Rejected Alternatives

### A. `impl-plan` group に加える

`obligations.json` を track 成果物として `impl-plan` group の pattern に加える案。
scope 分類の対象に留まるため、再導出のたびに `impl-plan` scope のレビューが失効する問題が残る。
`other` から移すだけで原因が変わらないため、採用しない。

### B. `other` のまま維持する

現状維持。
再導出のたびに `other` scope のレビューが失効し、判定対象のない再レビューを強いる。
その再レビューは reviewer のラウンド間非決定性を持ち込むだけであり、採用しない。

### C. `test-bindings.json` も同時に `review_operational` へ加える

義務まわりの track 成果物をまとめて運用扱いにする案。
binding 記録は書き手の authoring であり、レビューを迂回させると義務の充足判断が人手のレビューから外れる。
両者の性質の違いを無視するため、採用しない。

## Consequences

### Positive

- 再導出が scope hash を動かさなくなり、判定対象のない再レビューが消える。
- `review_operational` の登録基準が「機械導出か否か」で一貫する。

### Negative

- `obligations.json` の内容に対する人手のレビュー機会が失われる。導出の誤りは導出元か導出ロジックのレビューで捕捉する必要がある。
- 導出ロジックが将来 authoring を含むようになった場合、この分類は誤りになる。

### Neutral

- `test-bindings.json` の扱いは変わらない。
- commit gate の `test-obligation check` は scope 分類と独立に動作し、変更されない。

## Reassess When

- `obligations.json` の導出に人手の入力が混じるようになったとき。
- 導出結果の誤りが、導出元のレビューを通過して実害を生んだとき。
- `review_operational` の登録基準が「機械導出」以外の軸を必要としたとき。

## Related

- `.harness/config/review-scope.json`
- `.harness/workflows/track/type-design.md`
- `knowledge/conventions/enforce-by-mechanism.md`
