---
adr_id: "2026-08-23-0000-catalogue-lint-chain3-responsibility-boundary"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:2026-08-25:closed-spelling-grammar-adjudication"
    status: proposed
---
# カタログ型参照の静的検査と実装突合の責務を分離する

## Context

カタログの `TypeRef` には、カタログ内の型・トレイト参照と外部 Rust パスが同居する。`catalogue-lint` はカタログ文書だけを入力とするため、次の二つを同じ保証として扱えない。

- `TypeRef` 中の型・トレイト参照が、カタログの宣言エントリを一意に指しているか。
- wrapper、外部 path、型引数を含む `TypeRef` 全体が、実装に存在する Rust の型を表しているか。

以下「カタログ内参照」は、評価対象カタログ群の有効な型・トレイト宣言を指す参照をいう。generic parameter、lifetime、const 値、associated item のラベルは含まない。

## Decision

### D1: カタログ内参照と `TypeRef` 全体を別の判定根拠で fail-closed に検証する

`catalogue-lint` は有効な型・トレイト宣言の集合を判定根拠とし、`TypeRef` 内の全カタログ内参照を、各宣言の受理表記（宣言のキー、`module_path` と名前の結合形、crate 修飾の完全修飾形）との完全一致で判定する。複数の宣言に一致する曖昧な参照がある場合は候補の完全修飾パスを示して fail-closed とする。

どの宣言の受理表記にも一致しない参照はカタログ内参照ではなく、`catalogue-lint` はこれを検証せずに通す。その参照の存在と正しさは Chain ③ が実装との突合で検証する。

`catalogue-lint` が成功できるのは、`TypeRef` 全体の検査完了を確認できた場合だけである。未対応構文、解析不能、深さ・資源上限などにより検査を完了できなければ、その箇所を示して fail-closed とする。

Chain ③ は実装から得た型情報を判定根拠とし、外部 path を含む `TypeRef` 全体の適合を独立に fail-closed で検証する。一方の成功で他方の失敗や未検証を補ってはならない。

抽出・解決・照合の具体的方法と層間構成は、この ADR では固定しない。

#### Scope

対象は `TypeRef` を扱う `catalogue-lint` と Chain ③ の保証境界である。型参照の記法、完全修飾パスによる型・トレイト識別の定義、parser・port・adapter・依存注入は scope 外とする。

## Rejected Alternatives

- **`catalogue-lint` だけで `TypeRef` 全体を検証する**: カタログ文書だけでは外部パスの実在を判定できない。
- **全検査を Chain ③へ送る**: カタログ内で確定できる不整合の検出が遅れる。
- **部分的な解決を成功とする**: 未解決・分類不能な参照を隠し、fail-closed にならない。

## Consequences

- カタログ内の不整合は早期に、実装にしか確定できない不整合は Chain ③で検出できる。
- `catalogue-lint` が成功しても Chain ③で失敗し得る。両方の成功がその `TypeRef` の受理条件となる。
- 実装方法は下流設計に委ねる。

## Reassess When

- `catalogue-lint` が実装から得た型情報も判定根拠にする場合。
- `TypeRef` 全体の実装照合を別の gate が引き継ぐ場合。

## Related

- `knowledge/adr/2026-08-21-0055-type-identity-fully-qualified-paths.md#D1` — 完全修飾パスによる型・トレイト識別、短名の曖昧性、および fail-closed の基本方針。本 D1 は、その検証責務を `catalogue-lint` と実装突合に分ける。
