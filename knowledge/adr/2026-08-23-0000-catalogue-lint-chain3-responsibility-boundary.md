---
adr_id: "2026-08-23-0000-catalogue-lint-chain3-responsibility-boundary"
decisions:
  - id: D1
    review_finding_ref: "T008 domain review: catalogue-lint と Chain ③ の責務境界"
    status: proposed
---
# catalogue-lint と Chain ③ の型式検証責務を分離する

## Context

User は 2026-08-23 に、「`TypeRef` はゆるく評価され、実装との突合時に Rust の表現としての正しさを保証されればよいという前提が抜け落ちていそうです。`ModulePath` も同様です。」と述べた。これは catalogue 表記と実装突合の境界を見直す出発点であり、以下の具体的な責務分担そのものについての User の承認ではない。

T008 の domain review は、`std::vecc::Vec<OrderPlaced>` のように外部 crate の wrapper path が誤っていても、内側の catalogue 宣言型だけで catalogue-lint の規則が成功し得ることを、AC-04 / CO-03 の fail-closed 要求との矛盾として指摘した。一方、catalogue-lint は catalogue 文書だけを入力として実装突合より前に実行され、外部 crate path を照合する rustdoc 情報を持たない。

rollback-diagnoser は、この不整合の最上流を仕様上の責務境界の曖昧さと判断した。本 D1 の具体的な分担は、上記の User 発言を前提に、同診断と T008 の review finding から導出した候補である。

型式から catalogue identity を抽出する手段の選択は User の発言・承認ではない。P1 指摘を解消するための orchestrator の判断として、既存の Rust 型式 parser authority への委譲を採る。

## Decision

### D1: catalogue identity は catalogue-lint、Rust 型式全体は Chain ③ が fail-closed で検証する

catalogue-lint の universe は catalogue の宣言 entry 集合とする。この universe 内で catalogue identity が曖昧または未解決である場合、lint は候補の完全修飾パスを含む診断を出して fail-closed とする。外部 wrapper の綴りを含む Rust 型式全体の妥当性は catalogue-lint の検証対象に含めない。

外部 wrapper の綴りを含む Rust 型式全体の妥当性は、syn と rustdoc paths を持つ Chain ③ の実装突合が fail-closed で検証する。したがって、`std::vecc::Vec<OrderPlaced>` のような型式は、外部 path の誤りも含めて Chain ③ で不成立として扱う。

catalogue-lint は、有効な型式から内部の catalogue identity を正確に抽出しなければならない。構文マーカーや lifetime を path と誤認せず、外部 wrapper の内側にある宣言型まで到達して、catalogue 宣言 entry universe 内で解決する。この抽出は外部 wrapper 自体の Rust としての正しさを lint が保証することを意味しない。

この抽出のために catalogue-lint は Rust 型式の構文解析を自前で行わず、syn ベースの既存 Rust 型式 parser authority に委譲する。domain は抽出の port を定義し、infrastructure はその adapter を実装し、composition root は adapter を注入する。domain は Rust 型文法を再実装しない。

#### Scope

- 仕様の `IN-03`、`IN-04`、`AC-04`、`AC-07`、`CO-03`、`OS-01` における catalogue-lint と Chain ③ の責務境界。
- catalogue-lint の `ReferencedRoleConstraint`、`FieldElementUniqueAcrossEntries`、`NoExternalReferenceInMethods` の三規則。
- Chain ③ の syn と rustdoc paths を用いる実装突合。
- catalogue identity 抽出の domain port、syn ベース parser を用いる infrastructure adapter、およびその composition root 配線。

## Rejected Alternatives

- **catalogue-lint に Rust 型式全体の妥当性を検証させる**: 外部 crate path を照合する rustdoc 情報を持たない pre-implementation gate に、その入力だけでは判定不能な責務を与えることになる。
- **外部 wrapper を無視した成功を許す**: 型式全体を検証できる Chain ③ に到達しても wrapper の誤りを見逃し、実装突合の fail-closed を失わせる。
- **保守的 over-approximation で内部 identity を推定する**: lifetime、`mut`、`*const`、外部 wrapper の取り扱いで繰り返した回帰を避けられず、正確な抽出の根拠にならない。
- **domain で深さ見積りを伴う strict parser を実装する**: Rust 型文法を domain に再実装することになり、構文解析を adapter が所有する境界と既存 authority への集約に反する。

## Consequences

- 良: catalogue-lint は注入された抽出 port を通じて catalogue 宣言 entry universe における identity 解決を完全に扱い、曖昧・未解決を候補完全修飾パス付きで拒否できる。
- 良: Rust 型式全体の正しさは、それを構文解析し rustdoc paths と照合できる Chain ③ で一貫して検証される。
- 負: lint と Chain ③ の双方で、同じ外部 wrapper 誤記を同じ段階で検出することは期待しない。診断の責務と到達点を検証ごとに明確に保つ必要がある。
- 負: lint の組み立てでは parser adapter の注入が必要になり、domain と infrastructure の境界を迂回する直接依存は許容されない。

## Reassess When

- catalogue-lint が rustdoc paths を利用し、外部 Rust path を照合できる入力と責務を持つようになるとき。
- Chain ③ が Rust 型式の構文解析または rustdoc paths による照合を行わなくなるとき。

## Related

- `knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1` — 本 D1 は、catalogue identity の完全修飾パス解決に関する同決定を、catalogue-lint と Chain ③ の責務境界について refine する候補である。
