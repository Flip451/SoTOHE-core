---
adr_id: "2026-09-04-0057-catalogue-schema-declaration-guidance"
decisions:
  - id: D1
    user_decision_ref: "consumer handoff agent-router 2026-09-04; fleet adjudication 2026-09-04"
    status: proposed
  - id: D2
    user_decision_ref: "consumer handoff agent-router 2026-09-04; fleet adjudication 2026-09-04"
    status: proposed
---
# catalogue の宣言規則を文書化し、位置引数の鍵付けを field item に限る

## Context

consumer プロジェクトが catalogue を書く際、2 つの宣言法がスキーマ文書(`.harness/reference/catalogue-schema.md`)に無く、試行錯誤を要した。(1) lifetime 引数を持つ trait の impl(`impl<'de> Deserialize<'de>`)は `trait_ref` に引数を含め、かつ `impl_generics` に先頭 `'` なしの名前を宣言しないと 🔵 にならない。(2) private field を持つ tuple struct の inherent method をトップレベル `inherent_impls` に置くと、構造照合が impl 由来の子要素にも位置番号 `[0]` を付けるため実装側と鍵が食い違い 🟡 になる — entry 本体の `methods` に置けば 🔵。

(1) は評価器が lifetime 名の `'` あり/なしを意図的に同一視している(`signal_evaluator_v2/format/ty_canon.rs`、`impl_identity_helpers.rs`)ので、既存挙動の文書化で足りる。ただし domain 側 DTO の doc コメント(`catalogue_v2/entries.rs` と `catalogue_v2/traits.rs` の `impl_generics`)は「lifetime は対象外」と述べており、文書だけ直すと source コメントと矛盾が残る。

(2) の原因は 3 つの挙動の組み合わせである。catalogue codec は entry 本体の `methods` が空でも inherent impl を 1 つ作って型に繋ぐため、`inherent_impls` 併用時は catalogue 側の impl 子要素が 2 つ、rustdoc 側は 1 つになる。catalogue codec は private field を末尾 `None` 1 個で表す。そして構造照合の子要素列挙は `None` を落としてから impl を後ろに繋ぐ一方、位置番号の判定は落とす前の field 数と子 index を比べる(`structural_eq.rs` の `child_item_ids` と `positional_child`)。結果、catalogue 側では空 impl が `[0]` を得て method 入り impl は番号なし、rustdoc 側では唯一の impl が `[0]` を得て、method の identity 鍵が食い違う。位置番号が impl 子要素に付くのは private field があるときだけであり、全 field 公開の tuple struct では impl の index が field 数に等しく番号は付かない。

(2) は位置番号の付与が struct field 以外にも及ぶ照合側の不整合であり、文書で回避を案内するだけでは宣言者の負担が残る。

また、catalogue codec が private field を末尾 `None` 1 個で表すのに対し rustdoc は private field ごとに `None` を置くため、private field が 2 つ以上、または public field より前に private field がある tuple struct は、鍵付けを直しても field 列の長さ不一致で 🔵 にならない。この制約は本 ADR の範囲外だが、宣言者が知らずに試行錯誤する原因になるので案内に載せる。

## Decision

### D1: スキーマ文書と DTO doc コメントに宣言規則を明記する

`catalogue-schema.md` に次を可否の実例つきで追記する。

- lifetime 引数つき trait impl の宣言法: `trait_ref` は引数を含めた表記(`serde::Deserialize<'de>`)とし、`impl_generics` には lifetime 名を先頭 `'` なし(`de`)で宣言する。
- inherent method の置き場: entry 本体の `methods` が正規。`inherent_impls` は entry 本体で表せない場合(impl-block-level generics や where 条件つき等)に限る。
- tuple struct の private field: 照合可能なのは「末尾に private field が 1 つ」の形のみ。private field が 2 つ以上、または public field より前にある tuple struct は現行機構では 🔵 にならない。

あわせて `catalogue_v2/entries.rs` と `catalogue_v2/traits.rs` の `impl_generics` doc コメントから「lifetime は対象外」の記述を外し、スキーマ文書と同じ規則(lifetime 名は `'` なしで宣言する)に揃える。

### D2: 位置番号による子要素の鍵付けは、子要素が field item であるときに限る

構造照合の位置番号(`[0]` 等)は、子要素自身が tuple struct または tuple variant の field item であるときにのみ付与し、子 index と field 数の比較では決めない。impl 由来の子要素には付与しない。これにより `inherent_impls` 配置でも実装側と鍵が一致する。回帰テストとして、末尾に private tuple field を 1 つ持つ型の inherent method を `inherent_impls` に置いた fixture で 🔵 になることを固定する。

## Rejected Alternatives

- **文書化のみで D2 を行わない**: 正当な宣言形が照合側の都合で 🟡 になる状態が残り、consumer が理由の分からない失敗に当たり続ける。
- **`inherent_impls` を廃止して `methods` に一本化**: 既存 catalogue の移行と、entry 本体で表せない impl(型パラメータ条件つき等)の受け皿の喪失を招く。
- **D2 を「impl 由来の子要素を除外する」と書く**: tuple variant の field も同じ関数で番号を得ており、除外列挙では読み手に variant の扱いが曖昧になる。述語を「子要素が field item か」に置けば実装の条件式と一致する。
- **private field 複数の tuple struct 照合を本 ADR で解消する**: codec の `None` 符号化と照合の長さ比較の両方に及び、本 ADR の主題(宣言規則と鍵付け)を超える。Reassess When に記録する。

## Consequences

- 良: consumer が試行錯誤していた 2 点が文書と機構の両面で解消する。private field 複数の制約も案内に載り、宣言者が事前に知れる。
- 中立: D2 は構造照合の 1 関数の適用条件の変更。判定が変わるのは「tuple struct かつ private field あり かつ inherent impl」の組だけで、既存の 🔵 判定に影響しないことを回帰で確認する。
- 中立: D1 の DTO doc コメント改訂は挙動を変えない。

## Reassess When

- catalogue schema の改版で `inherent_impls` の意味論が変わったとき。
- private field が 2 つ以上、または public field より前にある tuple struct の照合が必要になったとき(codec の末尾 `None` 1 個の符号化と、照合側の長さ比較を併せて見直す)。
