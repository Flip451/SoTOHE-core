---
adr_id: "2026-09-04-0049-rustdoc-child-exclusion-by-kind"
decisions:
  - id: D1
    user_decision_ref: "consumer handoff agent-router 2026-09-04 (id0 defect report with patch); fleet adjudication pending"
    status: proposed
---
# rustdoc 子アイテムの除外は番兵 id ではなく kind で判定する

## Context

型シグナル評価器の構造照合は、子アイテムの id が `Id(0)` のとき「`Self` 番兵または crate root」とみなして走査を省く。現行の rustdoc JSON(format_version 57)は `Id(0)` を通常のアイテム(struct field 等)に割り当てるため、その 1 要素が実装側の参照集合から欠落し、catalogue と食い違って 🟡 になる。catalogue の書き方では回避できず、consumer プロジェクトで 3 型が誤って 🟡 となった(sotp-v0.1.0)。

## Decision

### D1: crate root の除外は `ItemEnum::Module` の kind で判定し、id の値に意味を持たせない

子アイテムの走査で除外するのは、`index` 上で kind が Module であるもの(crate root を含む)のみとする。id の特定値を番兵として扱わない。rustdoc が id に与える意味は format ごとに変わりうるため、判定は rustdoc が明示する kind に委ねる。`Id(0)` を struct field に割り当てた固定 fixture で、参照集合が全 field を含むことを回帰テストとして固定する。

## Rejected Alternatives

- **format_version で分岐して旧仕様のみ番兵扱いする**: rustdoc の内部規約への依存を温存し、次の format 変更で再発する。
- **catalogue 側で当該 field の宣言を省く**: 実装側の欠落を宣言側で隠す fail-open。

## Consequences

- 良: consumer で誤 🟡 になっていた型が 🔵 に戻る。番兵 id という暗黙の前提が消える。
- 中立: 修正は 1 関数・数十行。既存テストの `Id(0)` 使用(型パスの placeholder)は子アイテム走査と無関係で影響しない。

## Reassess When

- rustdoc JSON が crate root や `Self` を kind 以外の手段で表すようになったとき。
