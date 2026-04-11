# 型アクション宣言 — add / modify / delete の意図表明 (TDDD-03)

## Status

Proposed

## Context

ADR `2026-04-11-0001-baseline-reverse-signals.md` (TDDD-02) で、baseline と宣言カタログの集合関係による 4 グループ評価が導入された。

グループ 3 (B\A: baseline にあり宣言なし) で型が削除されたケース (C に存在しない) は **Red** として検出される。しかし、意図的な削除と事故的な削除を区別する手段がない。TDDD-02 では既存型の削除を含む track で `/track:design` を使用しない制約を設けているが、削除と TDDD を併用できないのは制限が強い。

同様に、既存型の変更 (A∩B) と新規型の追加 (A\B) も、カタログ上では区別されていない。宣言に `action` を持たせることで、開発者の意図を明示的に記録できる。

## Decision

`domain-types.json` の各エントリに optional な `action` フィールドを追加する。

```json
{
  "name": "Order",
  "kind": "typestate",
  "action": "add",
  "description": "New order aggregate",
  "approved": true
}
```

### `action` の値と評価ロジック

| `action` | 意味 | forward check |
|---|---|---|
| `"add"` (デフォルト、省略時) | 型を新規追加する | C に存在し宣言と一致 → Blue |
| `"modify"` | 既存型を変更する | C に存在し宣言と一致 → Blue |
| `"reference"` | 既存型をそのまま参照目的で宣言する | C に存在し宣言と一致 → Blue |
| `"delete"` | 型を意図的に削除する | C に **存在しない** → Blue、C に存在する → Yellow (まだ削除されていない) |

`"add"`, `"modify"`, `"reference"` の forward check ロジックは同一。区別する理由は意図の記録と、将来の検証精度向上:

- `"add"` + baseline にある (A∩B) → 矛盾: 「新規追加」と宣言したが既に存在する。警告を出せる
- `"modify"` + baseline にない (A\B) → 矛盾: 「変更」と宣言したが元の型がない。警告を出せる
- `"reference"` + baseline にない (A\B) → 矛盾: 「参照」と宣言したが元の型がない。警告を出せる
- `"reference"` + 宣言と実装が不一致 → 矛盾: 「そのまま転記」と宣言したが差異がある。警告を出せる

`"reference"` の用途:
- 生成される md ファイルの可読性のために既存型を転記する

### グループ 3 (B\A) への影響

`action: "delete"` を宣言すると、その型は A に入る (宣言済み)。したがって B\A ではなく A∩B に分類され、forward check で「C に存在しない → Blue」と評価される。

これにより:
- 宣言なしの削除 → Red (TDDD-02 の現行動作を維持)

**実装時の注意**: `action: "delete"` 宣言時は、当該型が baseline (B) に存在することを検証し、存在しない場合は**エラー**とする。baseline に存在しない型に対する `"delete"` 宣言は、存在しない型の削除を成功と偽る穴になるため、警告ではなくエラーで阻止する。
- `action: "delete"` 宣言ありの削除 → Blue (意図的)

## Rejected Alternatives

### A. baseline に削除済みフラグを持たせる

`domain-types-baseline.json` にエントリごとの `"deleted": true` フラグを追加する案。

却下理由:
- baseline はコードの事実のスナップショットであり、開発者の意図を持つべきではない
- 意図の宣言はカタログ (`domain-types.json`) の責務

### B. 既存型の削除を含む track では `/track:design` を使わない制約を維持する

TDDD-02 の現行制約をそのまま使う案。

却下理由:
- 削除とそれ以外の型変更が同じ track に含まれるケースで、TDDD の恩恵を受けられない
- 削除の意図が git 履歴に残らない (`domain-types.json` に `action: "delete"` があれば意図が明確)

## Consequences

### Good

- **意図の明示**: 追加・変更・削除の 3 種の意図がカタログに記録される
- **矛盾検出**: `action` と baseline の集合関係の不整合 (例: `"add"` だが baseline に既存) を警告できる
- **TDDD-02 の制約解消**: 既存型の削除と TDDD を同一 track 内で併用可能になる
- **後方互換性は対応しない**: 既存の `domain-types.json` のマイグレーションは行わない。TDDD-03 実装後に開始する新しい track から `action` フィールドを使用する。完了済み track の `domain-types.json` は TDDD-03 のコードが読むことはないため、`action` 省略時のデフォルト `"add"` が既存エントリに適用される状況は起きない

### Bad

- **`DomainTypeEntry` の拡張**: フィールド追加 + codec 変更
- **forward check の分岐追加**: `"delete"` の場合は「存在しないこと」を確認する逆ロジック
- **概念の増加**: 開発者が `action` フィールドの存在を知る必要がある

## Reassess When

- 現行 4 値 (add/modify/reference/delete) 以外の `action` が必要になった場合 (例: `"rename"` — 旧名と新名のペア宣言)
- TDDD-01 (ADR `2026-04-11-0002`) の多層化後、`action` が層をまたぐ操作 (例: domain → usecase への型移動) に対応する必要が出た場合

## Related

- **ADR `2026-04-11-0001-baseline-reverse-signals.md`** (TDDD-02): 4 グループ評価の導入元。本 ADR はグループ 3 (B\A) の削除ケースに対する宣言的な解決策
- **ADR `2026-04-11-0002-tddd-multilayer-extension.md`** (TDDD-01): 型カタログの多層化。`action` フィールドはリネーム後の `TypeCatalogueEntry` にも適用される
