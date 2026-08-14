---
adr_id: 2026-07-04-0525-catalogue-v2-entry-lint-conformance
decisions:
  - id: D1
    user_decision_ref: "chat:2026-07-04 「TypeEntry (catalogue_v2) の lint 適合 refactor をこのトラックでやる」"
    candidate_selection: "from:[lint設定のentry allowlist緩和, entry型のlint適合refactor, schema拡張の取り下げ, delta-modify宣言形式の新設] chose:entry型のlint適合refactor"
    status: proposed
  - id: D2
    user_decision_ref: "chat:2026-07-04 hearing 「D2–D5 全承認」(D1 承認済みの lint 適合 refactor の下位決定として範囲/非公開化+constructor/docs newtype/同一変更集合 landing を一括承認)"
    status: proposed
  - id: D3
    user_decision_ref: "chat:2026-07-04 hearing 「D2–D5 全承認」(D1 承認済みの lint 適合 refactor の下位決定として範囲/非公開化+constructor/docs newtype/同一変更集合 landing を一括承認)"
    status: proposed
  - id: D4
    user_decision_ref: "chat:2026-07-04 hearing 「D2–D5 全承認」(D1 承認済みの lint 適合 refactor の下位決定として範囲/非公開化+constructor/docs newtype/同一変更集合 landing を一括承認)"
    status: proposed
  - id: D5
    user_decision_ref: "chat:2026-07-04 hearing 「D2–D5 全承認」(D1 承認済みの lint 適合 refactor の下位決定として範囲/非公開化+constructor/docs newtype/同一変更集合 landing を一括承認)"
    status: proposed
---

# catalogue_v2 エントリ型の catalogue linter 適合 refactor

## Context

catalogue_v2 の entry 型（`libs/domain/src/tddd/catalogue_v2/entries.rs` の `TypeEntry` / `TraitEntry` / `FunctionEntry`）は、カタログの type / trait / function エントリを表すデータレコードである。いずれも全フィールドが `pub` で、ドキュメント文字列を `docs: Option<String>` という生 primitive で保持する。この形状はカタログ段階 linter（`.harness/catalogue-lint/config.json`）の 2 ルールが導入される前から存在しており、これまで lint 対象になっていなかった。

カタログ linter の評価は entry の `action` を見て対象を絞る。`action: Reference` と `action: Delete` のエントリだけが lint から除外され、`action: Add` と `action: Modify` のエントリは宣言された最終形状が lint される（`libs/domain/src/tddd/catalogue_linter_helpers.rs`）。

型カタログに完全な型情報を維持するという方針（ADR `2026-07-02-1345-catalogue-generation-annotation.md` D1）と、宣言レベルの generics / where 句に正規フィールドがなければ同じ変更内でフィールドを追加するという方針（同 D6）により、`TypeEntry` に型宣言レベルの `generics` / `where_predicates` フィールドを追加する必要がある。TDDD の規律では、この追加を `action: Modify` のカタログエントリとして宣言する。すると linter は追加後の形状を lint し、カタログ段階では解消できない 2 件の違反を報告する。

- `NoPublicField`: `TypeEntry` は全フィールドが `pub` である。このルールは `ValueObject` / `DomainEvent` を対象とする。`TypeEntry` の正直な role は「behavior を持たない検証済みデータレコード」= `ValueObject` であり、この role の下でルールが発火する。
- `ForbidPrimitiveInTypes`: `docs: Option<String>` の生 `String` が named field 位置で検出される。このルールの named field 版は domain 層で合法な role をほぼ網羅して対象にしており、role の選び方で回避できない。

この行き詰まりはカタログ側の宣言では解けない。linter が除外するのは `Reference` / `Delete` action だけであり、`Modify` は必ず lint される。`NoPublicField` を避けるために `TypeEntry` を `Entity` などの別 role で宣言することは、`TypeEntry` が Entity でない以上、偽りの契約を書くことになる（`knowledge/conventions/type-designer-kind-selection.md` R5 の catch-all / semantic stretch 禁止に反する）。かつその role でも `ForbidPrimitiveInTypes` は `docs` の生 `String` で発火し続ける。差分だけを宣言する部分 modify の形式は現行の signal evaluator に存在せず、Modify は完全形状一致を要求する。

同じ legacy shape（全 `pub` フィールド + 生ドキュメント文字列）は `TraitEntry` / `FunctionEntry` も共有する。`TraitEntry` は過去のフィールド追加が linter 導入前に行われたため grandfather されているだけで、次に modify を宣言した時点で同じ行き詰まりが再発する。

## Decision

### D1: entry 型をカタログ linter に適合する形へ refactor する

lint 設定を緩めるのではなく、`catalogue_v2` の entry 型そのものを `NoPublicField` / `ForbidPrimitiveInTypes` に適合する形へ refactor する。カタログ段階 linter はプロジェクトが強制する検査であり、その検査面（完全な型情報）と検査基準（primitive obsession 防止・カプセル化）は維持したまま、被検査対象の側を規約に合わせる。

### D2: 対象範囲は同じ legacy shape を共有する全 entry 型とする

refactor の対象は `TypeEntry` だけでなく、同じ legacy shape（全 `pub` フィールド + 生ドキュメント文字列）を共有する `catalogue_v2` の entry 型（`TypeEntry` / `TraitEntry` / `FunctionEntry`、および同型を持つ他のエントリ型）とする。`TypeEntry` だけを直すと、`TraitEntry` / `FunctionEntry` の次の modify で同じ行き詰まりがそのまま残る。行き詰まりを一度で解体するため、範囲を entry 型全体に広げる。

### D3: フィールドは非公開にし、検証付き constructor と read アクセサで扱う

entry 型のフィールドを非公開にし、`NoPublicField` に適合させる。構築は全フィールドを受け取る constructor 経由に一本化し、読み出しは read アクセサ（getter）で行う。setter は設けない（entry はカタログ codec / 生成 API が一度組み立てた後は read-only として参照される）。

現在、codec（`libs/infrastructure/src/tddd/catalogue_document_codec/`）はクレート境界を越えて struct literal で entry を構築し、フィールドを直接読み出している。フィールドを非公開にすると struct literal 構築は成立しなくなるため、構築点を constructor に、読み出し点をアクセサに移す。具体的な constructor 引数順やアクセサの返り値型は型カタログ段階（Phase 2）で確定する。

### D4: `docs` は生 String をやめてドキュメント文字列の newtype にする

`docs` の生 `String` を、ドキュメント文字列を表す newtype で置き換え、`ForbidPrimitiveInTypes` に適合させる。lint 側の緩和（ルールから当該フィールドを除外する等）は D1 の方針により採らない。newtype はドキュメント文字列という不透明な自由テキストの薄いラッパーとし、newtype 自体もカタログ規約（`ValueObject`）に沿って宣言する。newtype の名前・検証内容は型カタログ段階で確定する。

### D5: refactor はフィールド追加と同一の変更で先行または同時に landing する

lint 適合 refactor は、行き詰まりの引き金となるフィールド追加（`TypeEntry` への型宣言レベル generics / where 句など）と同一の変更集合の中で、先行して、または同時に landing する。フィールド追加を `Modify` として宣言した時点で追加後の形状が lint されるため、その時点で entry 型が既に lint 適合していなければ違反が出る。refactor をフィールド追加と分離して後回しにはしない。作業単位への分解自体は実装計画（Phase 3）の領分であり、本 ADR は landing の順序制約のみを定める。

## Rejected Alternatives

- **lint 設定に entry allowlist を足して緩和する** (`.harness/catalogue-lint/config.json` に当該 entry 型を除外する allowlist を追加): ユーザーが明示的に却下。かつ、最小限の抜け穴で primitive obsession を防ぐという catalogue-primitive-obsession-guard の設計意図（ADR `2026-07-01` 系）に反し、恒久的な穴を lint に開けることになる。
- **フィールド追加（schema 拡張）を取り下げる**: 型宣言レベルの generics / where 句をフィールドとして追加せず行き詰まり自体を回避する案。完全な型情報を維持するという方針（ADR `2026-07-02-1345-catalogue-generation-annotation.md` D1）を破り、型宣言レベルの generics / where 句がカタログで表現できないまま残る。却下。
- **差分だけを宣言する delta-modify の宣言形式を新設する**: `Modify` を完全形状一致ではなく差分宣言で受けられるよう evaluator を拡張する案。signal evaluator の Modify 意味論（完全形状一致）を広範に変更することになり、行き詰まりの解消という目的に対して波及が大きすぎる。却下。
- **別 role で宣言して `NoPublicField` を回避する**: `TypeEntry` を `Entity` 等の `NoPublicField` 対象外 role で宣言して回避する案。`TypeEntry` は Entity ではないため偽りの契約になり、catch-all / semantic stretch 禁止（`knowledge/conventions/type-designer-kind-selection.md` R5）に反する。かつその role でも `ForbidPrimitiveInTypes` は `docs` の生 `String` で発火し続ける。却下。

## Consequences

### Positive

- フィールド追加を `Modify` として宣言する経路の行き詰まりが解ける。
- `TypeEntry` だけでなく `TraitEntry` / `FunctionEntry` の将来の modify でも同じ行き詰まりが起きなくなる（範囲を entry 型全体に広げたため）。
- entry 型の形状がプロジェクトの lint 基準（カプセル化・primitive obsession 防止）に揃い、grandfather された例外が減る。
- 構築点が検証付き constructor に一本化され、entry の組み立てが単一の検証地点を通るようになる。

### Negative

- 波及範囲が広い。フィールドを非公開にすることで、struct literal 構築とフィールド直読みに依存する全消費者（codec の decode / encode、linter helpers、signal evaluator、contract-map / type-graph の renderer、これらのテスト）を constructor + アクセサ経由へ移す必要がある。churn は小さくない。
- ドキュメント文字列の newtype が 1 つ増え、その newtype 自体のカタログ宣言も要る。
- entry 型のコンストラクタ / アクセサという新しい API 面が保守対象に加わる。

### Neutral

- lint の検査面・検査基準は変わらない。本決定は lint への適合であって緩和ではない。
- entry 型が保持する情報量は変わらない（可視性と `docs` の型が変わるだけ）。

## Reassess When

- カタログ schema の権威が手書きの entry 型定義から sotp のコード生成側へ移り、entry 型の形状が別機構で決まるようになった場合 — 手書き entry 型に対する本 refactor の前提が変わる。
- カタログ linter のルール集合（`NoPublicField` / `ForbidPrimitiveInTypes` の対象 role や position）が変わった場合 — 適合の必要範囲を再確認する。

## Related

- `knowledge/adr/2026-07-02-1345-catalogue-generation-annotation.md` — 完全な型情報の維持（D1）と宣言レベル generics / where 句の正規フィールド追加（D6）。本 refactor の引き金となるフィールド追加の根拠。
- `knowledge/conventions/type-designer-kind-selection.md` — role 選定規約（R3 ValueObject 制限 / R5 catch-all 禁止 / R9 primitive obsession 禁止）。
- `knowledge/conventions/prefer-type-safe-abstractions.md` — Newtype / Make Illegal States Unrepresentable（D4 の背景）。
- `knowledge/conventions/coding-principles.md` — カプセル化・命名・error handling の規約。
