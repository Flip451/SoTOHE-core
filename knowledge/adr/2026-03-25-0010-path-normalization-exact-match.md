# パス正規化: exact match + fail-closed for unknown paths

## Status

Accepted

## Context

RVW-11（diff スコープ強制）で、レビュアの finding に含まれる `file` フィールドと diff ファイルリストを照合する方式を決定する必要があった。

照合方式の選択肢:
1. Raw string exact match（正規化なし）
2. Normalized exact match（`./`, `\`, 絶対パスを正規化後に exact match）
3. Suffix match（`state.rs` → `libs/domain/src/review/state.rs` にマッチ）

## Decision

**Normalized exact match** を採用し、正規化不能なパスは **in-scope** として扱う（fail-closed）。

正規化ルール:
- 先頭の `./` を除去
- `\` を `/` に変換
- 絶対パスからリポジトリルートを strip

正規化不能なケース（例: `state.rs` のような短縮形）:
- in-scope として扱う（安全側に倒す）
- `unknown_path_count` としてカウントし stderr に報告

## Rejected Alternatives

- **Raw string exact match**: `./libs/foo.rs` と `libs/foo.rs` が不一致になり、in-diff finding が誤って除外されるリスク。
- **Suffix match**: monorepo で `mod.rs`, `lib.rs`, `types.rs` 等の同名ファイルが多数存在し、false positive（out-of-scope ファイルが in-scope と誤判定）が頻発する。安全側ではあるが、scope filtering の意味が薄れる。
- **Unknown paths を out-of-scope 扱い**: 正規化できなかった real finding が `zero_findings` に化ける致命的リスク。verdict を甘くする方向の誤りは許容できない。

## Consequences

- Good: fail-closed — verdict を甘くする方向には絶対に誤らない
- Good: `unknown_path_count` で正規化品質を可視化でき、改善の手がかりになる
- Bad: 正規化不能な out-of-scope finding は除外できない（in-scope として残る）

## Reassess When

- レビュアが一貫して正規化可能なパスを返すことが確認された場合、UnknownPath バリアントの扱いを見直す
- suffix match の安全なバリアント（例: 最低2セグメント一致を要求）が設計された場合
