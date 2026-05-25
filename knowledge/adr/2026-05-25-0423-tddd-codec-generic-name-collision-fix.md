---
adr_id: 2026-05-25-0423-tddd-codec-generic-name-collision-fix
decisions:
  - id: D1
    user_decision_ref: "chat_segment:tddd-codec-generic-name-collision-fix:2026-05-25"
    status: proposed
---
# 型シグネチャ codec の generic param 名前衝突の恒久対策

## Context

型シグネチャ codec（`libs/infrastructure/src/tddd/signal_evaluator_v2/format.rs` の `build_generic_canon_map` と、それを利用する `libs/infrastructure/src/tddd/signal_evaluator_v2/generics_eq.rs`）は generic param を「名前」をキーにした HashMap で管理している。rustdoc は `impl Trait` 引数を、その bound 文字列をそのまま名前とする合成 generic param に desugar する。このため `fn(a: impl Into<String>, b: impl Into<String>)` のような signature では、`"impl Into<String>"` という同じ名前の合成 param が 2 つ生成され、HashMap 上で衝突する（片方だけが残り、bound はマージされる）。

結果として、この種の signature を忠実に表現した catalogue 宣言は type-signals を Blue にできない。A 側（catalogue codec）は `Type::ImplTrait` をリテラル `"impl Into<String>"` として整形する一方、衝突で歪んだ C 側（rustdoc 由来）は位置 placeholder（`#1` 等）として整形されるため、構造比較が不一致（Yellow/Red）になる。`impl Trait` param が 1 個だけでも、ImplTrait リテラル vs 位置 placeholder の差で不一致になる。

この問題は reviewer-claude-option-2026-05-24 track（PR #138）で顕在化した。そこでは source を明示ジェネリクス（`fn new<M: Into<String>, B: Into<String>>(model: M, base_prompt: B)`）に書き換えて rustdoc が別名を出力するようにし、catalogue が `M`/`B` を忠実に宣言できる形にして回避した。これは個別 adopter の source 変更による回避であり、codec 本体は問題を抱えたままになっている。

関連する既存決定として、`2026-04-29-0240-method-type-full-generic-declaration.md` が method/param 型宣言で generic 引数を含む完全な型文字列を強制している。本 ADR はその「忠実な型宣言」を `impl Trait` 引数についても成立させるための codec 側の対応を扱う。

## Decision

### D1: 同名合成 param を位置で識別し、Type::ImplTrait を位置で描画する

`build_generic_canon_map`（および関連する canon map 構築）で、同名の合成 generic param を名前ではなく位置（index）で識別するように変更し、名前衝突を解消する。あわせて format 側で `Type::ImplTrait` を位置 placeholder として描画し、C 側（rustdoc の `Type::Generic`）と一致させる。

これにより、`impl Trait` 引数を持つ signature を明示ジェネリクスへ書き換えることなく忠実に catalogue 宣言でき、type-signals が Blue になる。adopter 側の source 回避は不要になる。

## Rejected Alternatives

### A. per-adopter の明示ジェネリクス回避を恒久策とする

`impl Trait` 引数を持つ全 adopter に対し、source を明示ジェネリクス（`<M: Into<String>>` 等）へ書き換えることを恒久的な運用ルールとする案。

却下理由: catalogue が言語仕様を忠実に表現できないという codec 側の欠陥を、利用者全員の source 変更で肩代わりさせることになる。`impl Trait` は Rust で一般的な書き方であり、それを使うたびに catalogue 都合の書き換えを強いるのは負担が大きく、忠実性の原則（`2026-04-29-0240`）にも反する。回避策としては有効だが恒久策にはしない。

### B. phantom-generic を catalogue に encode する

衝突する `impl Trait` を、catalogue 側で `A, B where B: Into<String> + Into<String>` のような phantom generic として encode し、構造比較を通す案。

却下理由: Blue にはなるが、実際の signature とは異なる不正確な contract を catalogue に書くことになる。reviewer がこの不一致を flag する（実際に PR #138 のレビューで指摘された）。catalogue は実装の忠実な契約であるべきで、信号を通すためだけの歪んだ表現を持ち込まない。

## Consequences

### Positive

- `impl Trait` 引数を持つ signature を、明示ジェネリクスへ書き換えずに忠実に catalogue 宣言でき、type-signals が Blue になる。
- adopter 側の source 回避（明示ジェネリクス化）が不要になり、自然な Rust の書き方をそのまま維持できる。

### Negative

- 位置ベースのマッピング導入により、param の順序に依存する新たな不具合が入り込む余地があるため、テストで網羅する必要がある。

## Reassess When

- rustdoc が `impl Trait` 合成 param の命名規則を変更し、名前衝突がそもそも起きなくなったとき。
- signal_evaluator_v2 の canon map 構築を再設計するとき。
- 位置ベース識別の導入によって、param 順序に依存する新たな偽陰性/偽陽性が判明したとき。

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/adr/2026-04-29-0240-method-type-full-generic-declaration.md` — generic 引数を含む完全な型宣言の強制（本 ADR が `impl Trait` 引数について補完する）
- `libs/infrastructure/src/tddd/signal_evaluator_v2/format.rs` — `build_generic_canon_map`（修正対象）
- `libs/infrastructure/src/tddd/signal_evaluator_v2/generics_eq.rs` — `build_generic_canon_map` の利用箇所
- reviewer-claude-option-2026-05-24 track / PR #138 — 名前衝突が顕在化し、明示ジェネリクス化で回避した track
