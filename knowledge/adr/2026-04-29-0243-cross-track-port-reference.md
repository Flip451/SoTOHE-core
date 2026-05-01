---
adr_id: 2026-04-29-0243-cross-track-port-reference
decisions:
  - id: D1
    user_decision_ref: "chat_segment:tddd-struct-kind-spinoff:2026-04-29"
    status: proposed
---
# secondary_adapter が参照する port は当該 track の catalogue に必ず declare する

## Context

infrastructure 層の `secondary_adapter` が implement する trait が、
対象 track の `<layer>-types.json` に `secondary_port`
として declare されていない場合、contract-map renderer はその trait を
`port_index` で resolve できず、`secondary_adapter -.impl.-> port` edge を
silently skip する。

このパターンが発生するのは、別 track や workspace baseline 由来の
pre-existing port を当該 track の adapter が impl する場合である。
pre-existing port は当該 track では「変更しない」ため、type-designer が
declare 不要と判断しがちだが、catalogue への exposure (graph 描画) のためには
当該 track の catalogue にも entry が必要になる。

renderer の挙動:

- `secondary_adapter::implements[].trait_name` から trait 名を取り出す
- `port_index: BTreeMap<String, Vec<String>>` (当該 track の `secondary_port` 限定)
  で trait 名を lookup
- match しなければ `-.impl.->` edge を生成しない

結果として、`infrastructure-types.json::implements[]` の宣言と
`contract-map.md` の edges 集合の間に矛盾が発生する。

具体的な観測例:

- `FsReviewStore.implements[]` に `ReviewReader` / `ReviewWriter` を declare
- `domain-types.json` には `ReviewReader` / `ReviewWriter` の `secondary_port` entry が
  存在しない (baseline 由来)
- 結果として `FsReviewStore -.impl.-> ReviewReader` の edge が contract-map に出ない

## Decision

### D1: `secondary_adapter::implements[]` で参照する port は当該 track の catalogue に declare する

#### 規範

`secondary_adapter::implements[]` で参照する trait は、当該 track の `<layer>-types.json`
のいずれかに `secondary_port` として entry が必須である。

当該 track で改変しない baseline 由来の port は `action: "reference"` で declare し、
catalogue への exposure を確保する。

#### `action: "reference"` の semantics

- 当該 track では対象 port を変更しない (新規メソッド追加・既存メソッド変更なし)
- catalogue exposure (contract-map / graph 描画) を成立させるための declare
- type-signal evaluator は `reference` action に対して「完全一致のみ Blue、不一致はすべて Red」
  として評価する (modify の Yellow 吸収は適用されない)
- ADR `2026-04-26-0855` §S の信号機評価 table の `reference` action 列に従う

#### 実装上の制約

- 対象 trait: `secondary_port` (driven port)
- declare 漏れ: contract-map renderer の `port_index` lookup が unmatched となり、
  `-.impl.->` edge が silently skip される
- declare 義務: `secondary_adapter::implements[]` で trait_name を参照する以上、
  対応する port entry を当該 track 内に作成する責任は type-designer に帰属する
- baseline port の `expected_methods` は baseline 当時の全 method を列挙する
  (method 型宣言の完全形規範と同様、`reference` action でも completeness は要求される)

#### convention 連携

本 decision は `knowledge/conventions/type-designer-kind-selection.md` の
**R7 (Cross-Track Port Reference)** として運用ルール化する予定 (現時点では draft)。
type-designer agent は draft 段階から R7 violation を self-reject する義務を負う。
本 ADR は方針の決定記録として残し、convention が運用ルール化後に判断手順 / Examples /
Review Checklist を提供する SSoT として機能する。

## Rejected Alternatives

### A1: contract-map renderer が baseline workspace trait を自動補完する

`secondary_adapter::implements[].trait_name` のうち当該 track の `port_index` に
存在しないものを、workspace 全体の baseline から自動 resolve して edge を補完する案。

**却下理由**: 同名 trait が複数の layer / 複数の track に存在する場合、`trait_name` 単独
ではどの定義が参照されているのか一意に決まらない。`action: "reference"` による明示宣言の
方が曖昧性がなく、当該 track が「どの版の trait を参照しているか」をカタログから直接
読み取れる。auto-render は暗黙的な曖昧解決であり、「declare すれば edge、declare しなければ
orphan」という TDDD の原則と整合しない。

### A2: merge gate の integrity check のみで対応する

render 段階での edge 生成は現状維持 (port が declare されていなければ silently skip)、
merge gate で「`implements[]` の `trait_name` が当該 track または baseline に存在する」か
を verify するだけで済ます案。

**却下理由**: gate は事後検出 (close-out 時点での発覚) であり、graph を見ながら設計を
進める途中段階でのフィードバックが得られない。render 段階で edge 欠落を表面化させる方が
設計者の反応が早い。なお gate 側の integrity check 自体は補助的に有用であり、
後続の作業として実装することは妨げない。

## Consequences

### 良い影響

- baseline 由来の port を参照する adapter の `-.impl.->` edge が欠落しなくなり、
  cross-track 設計の接合点が contract-map 上で可視化される。
- `action: "reference"` の semantics が明確になり、type-designer が declare 義務を
  把握しやすくなる。

### 悪い影響・トレードオフ

- baseline 由来 port を持つすべての track で、adapter が参照する port の
  `reference` declare を追加する必要が生じる。過去 track の retrofit は不要だが、
  新規 track では type-designer に義務が追加される。

## Reassess When

- `action: "reference"` を使う entry が増えすぎて catalogue が見づらくなった場合:
  baseline port を一括 expose する別の仕組みを検討する。
- workspace 全体の baseline を一元管理する仕組みが整備された場合:
  auto-resolve の再検討余地が生まれる (A1 の再評価)。

## Related

- `knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md` —
  本 ADR の主テーマである struct kind 均質化の元 ADR。cross-track port reference は
  そこから spin-off した独立 decision。
- `knowledge/adr/2026-04-26-0855-tddd-feature-extension-with-verification.md` —
  `action: "reference"` の semantics を確立した前駆 ADR。
- `knowledge/conventions/type-designer-kind-selection.md` —
  本 ADR decision の運用ルール SSoT。R7 (Cross-Track Port Reference) として
  type-designer の判断手順 / Examples / Review Checklist を提供する。
