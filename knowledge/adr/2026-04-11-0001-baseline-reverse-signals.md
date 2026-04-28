---
adr_id: 2026-04-11-0001-baseline-reverse-signals
decisions:
  - id: 2026-04-11-0001-baseline-reverse-signals_grandfathered
    status: accepted
    grandfathered: true
---
# Baseline-Aware Reverse Signal Detection (TDDD-02)

## Status

Proposed

## Context

ADR `2026-04-08-1800-reverse-signal-integration.md` で reverse signal（code → spec）が導入された。未宣言型は Red シグナルとして `domain-types.json` に記録され、`verify spec-states` が CI でブロックする。

### 問題: 既存型ノイズ

reverse check は domain crate の全 pub 型を TypeGraph から取得し、`domain-types.json` に宣言されていない型を全て Red とする。しかし `domain-types.json` は **per-track** スコープであり、各 track は自分が新たに導入する型のみを宣言する。

結果:
- 成熟したコードベースでは 100+ の既存型が Red ノイズとして出力される
- 本 track で未宣言のまま追加された「本当に検出すべき型」がノイズに埋もれる
- `verify spec-states` が Red ゼロを要求するため、CI が通らない（既存型のせい）

### 問題の本質

reverse check の対象が「全型」であるべきではなく、「この track で変化した型」であるべき。具体的には:

1. **新規型**: この track で新しく追加された型 → 宣言が必要
2. **構造変更**: 既存型に variant/field/method を追加・削除した → 変更の宣言が必要
3. **不変型**: design 時点から構造が変わっていない既存型 → 無視すべき

## Decision

### 1. `/track:design` 時に TypeGraph の構造スナップショットをベースラインとして保存

`sotp track baseline-capture` コマンド（`/track:design` の Step 4 から呼ばれる）で、現在の TypeGraph を `domain-types-baseline.json` として track ディレクトリに保存する。このファイルは以降 `domain-type-signals` から read-only 参照される。

#### ファイル構造

ベースラインは **rustdoc JSON から構築された `TypeGraph` のスナップショット**であり、`TypeNode` / `TraitNode`（`libs/domain/src/schema.rs`）の構造を JSON にシリアライズしたもの。`domain-types.json`（宣言カタログ）とはスキーマが異なる。

TypeGraph の内部構造 (`HashMap<String, TypeNode>` / `HashMap<String, TraitNode>`) に合わせ、型名をキーとするオブジェクト形式を使用する:

```json
{
  "schema_version": 1,
  "captured_at": "2026-04-11T00:01:00Z",
  "types": {
    "TrackId": { "kind": "struct", "members": ["0"], "method_return_types": [] },
    "TaskStatus": { "kind": "enum", "members": ["Todo", "InProgress", "Done", "Skipped"], "method_return_types": ["TaskStatusKind"] },
    "ReviewInProgress": { "kind": "struct", "members": ["state"], "method_return_types": ["Approved", "Rejected"] }
  },
  "traits": {
    "TrackReader": { "methods": ["find"] },
    "TrackWriter": { "methods": ["save", "update"] }
  }
}
```

オブジェクト形式の利点:
- TypeGraph の `HashMap<String, _>` と 1:1 対応。serde の `HashMap` デシリアライズがそのまま使える
- 比較時に型名で O(1) ルックアップ。配列を走査して `name` フィールドで探す必要がない

フィールド定義:
- `types.<name>.kind`: `TypeKind` のシリアライズ値 (`"struct"` / `"enum"` / `"type_alias"`)
- `types.<name>.members`: `TypeNode::members` — enum なら variant 名、struct なら field 名（sorted）
- `types.<name>.method_return_types`: `TypeNode::method_return_types` — inherent impl メソッドの戻り値型名（sorted）
- `traits.<name>.methods`: `TraitNode::method_names` — trait が定義するメソッド名の配列（sorted）

除外フィールド（`TypeNode` の以下のフィールドはベースラインに含めない）:
- `outgoing`: `method_return_types ∩ typestate_names` の導出値。`method_return_types` は既に保存済みであり、`typestate_names` は `domain-types.json` から都度取得できるため冗長
- `module_path`: モジュール移動は構造変更ではなくリファクタリング。variant/field/method の変更ではないため比較対象に含めない

### 2. `domain-types-baseline.json` と `domain-types.json` の 2 ファイル構成

| ファイル | 役割 | ライフサイクル |
|---|---|---|
| `domain-types.json` (既存・変更なし) | 宣言済み型 + signals | `/track:design` で作成、`domain-type-signals` で signals 更新 |
| `domain-types-baseline.json` (新規) | design 時点の TypeGraph 構造スナップショット | `baseline-capture` で生成、以後 read-only |

`domain-types.json` のスキーマは変更しない。ベースラインは別ファイルに分離することで:
- 既存スキーマの migration が不要
- ライフサイクルが異なる（baseline は不変、declared types は evolving）ことが構造的に明確
- ベースラインが大きくなっても（100+ エントリ）declared types の可読性に影響しない

### 3. `check_consistency` の 4 グループ評価

`check_consistency()` は `&TypeBaseline` を新たに受け取る。現在の TypeGraph (C) の各型を、宣言 (A) と baseline (B) の集合関係で 4 グループに分類し、グループごとに評価する。

```
A = domain-types.json で宣言している型
B = baseline に含まれる型 (design 時点の既存型)
C = 現在の TypeGraph (コードに存在する型)
```

baseline の生成と評価は別コマンドに分離する (§4)。`domain-type-signals` は baseline が存在しない場合エラーとする。

#### グループ 1: A\B — 新規型（宣言あり・baseline なし）

これから実装する型。forward check のみ。baseline は無関係。

| C の状態 | Signal |
|---|---|
| 存在し、宣言と一致 | **Blue** |
| 存在するが、宣言と不一致 | **Yellow** (実装途中) |
| 存在しない | **Yellow** (未着手) |

#### グループ 2: A∩B — 既存型の宣言（宣言あり・baseline あり）

既存型をカタログに宣言した型。forward check のみ。宣言済みなので reverse check の対象外。

宣言の意図は 2 通りある:
- **変更目的**: 既存 enum に variant を追加する等、変更を意図して宣言
- **参照目的**: 生成される md ファイルの可読性や文脈のために、既存型をそのまま宣言

いずれの場合も forward check のロジックは同一（宣言と実装の比較）。意図の区別は ADR `2026-04-11-0003-type-action-declarations.md` (TDDD-03) の `action` フィールドで対応する。

| C の状態 | Signal |
|---|---|
| 存在し、宣言と一致 | **Blue** |
| 存在するが、宣言と不一致 | **Yellow** (変更途中) |
| 存在しない | **Yellow** |

#### グループ 3: B\A — 既存型・今回触らない（宣言なし・baseline あり）

今回の track で変更しない既存型。reverse check の対象。**本 ADR の主対象**。

| C の状態 | Signal |
|---|---|
| 存在し、baseline と構造同一 | **スキップ** (ノイズ除外) |
| 存在するが、baseline と構造が異なる | **Red** (未宣言の構造変更) |
| 存在しない (削除された) | **Red** (未宣言の削除) |

構造比較のセマンティクス:
- **型の比較**: `kind` + sorted `members` + sorted `method_return_types` の equality で判定。3 つのいずれかが異なれば構造変更とみなす
- **トレイトの比較**: sorted `method_names` の equality で判定

#### グループ 4: ∁(A∪B) ∩ C — 未宣言の新規型（宣言なし・baseline なし・コードにある）

baseline 後に追加されたが宣言されていない型。reverse check の対象。

| C の状態 | Signal |
|---|---|
| 存在する | **Red** (未宣言の新規型) |

#### まとめ

| グループ | check 種別 | Blue | Yellow | Red | スキップ |
|---|---|---|---|---|---|
| A\B (新規型) | forward | 宣言と一致 | 未着手/途中 | ─ | ─ |
| A∩B (既存変更) | forward | 宣言と一致 | 変更途中 | ─ | ─ |
| B\A (既存不変) | reverse | ─ | ─ | 構造変更 / 削除 | 構造同一 |
| ∁(A∪B)∩C (未宣言新規) | reverse | ─ | ─ | 常に Red | ─ |

**制約**: 本 ADR の実装後、ADR `2026-04-11-0003-type-action-declarations.md` (TDDD-03) が実装されるまで、既存型の削除を含む track では `/track:design` を使用しないこと (Red が解消できないため)。TDDD-03 の `action: "delete"` 実装後に、既存型の削除と TDDD を併用可能になる。

### 4. CLI コマンド分離

baseline の生成と signal 評価を別コマンドに分離する。

#### `sotp track baseline-capture <track-id>`

baseline 生成専用コマンド。`/track:design` の Step 4 から呼ばれる。

```
1. domain-types-baseline.json が既に存在する場合はスキップ（冪等）
2. rustdoc → TypeGraph (C) 構築
3. TypeGraph → TypeBaseline 変換
4. domain-types-baseline.json として保存
```

- `/track:design` から呼ばれる。再実行しても baseline が既に存在すればスキップする（冪等動作）。`/track:design` の再実行は宣言カタログ (A) の変更であり、コード (C) は変わらないため baseline (B) は stale にならない
- 意図的に再生成したい場合は `--force` フラグで上書き
- schema_version の互換性チェックは行わない。baseline の schema が変わるのは TDDD-01 の scope であり、その時点で新しい track を開始するため同一 track 内で schema が古くなることは起きない
- 生成後は design 成果物と一緒にコミットを推奨

#### `sotp track domain-type-signals <track-id>` (既存コマンドの拡張)

signal 評価専用コマンド。baseline を読み込んで 4 グループ評価を行う。

```
1. domain-types.json (A) 読み込み
2. rustdoc → TypeGraph (C) 構築 (既存)
3. domain-types-baseline.json (B) 読み込み → 存在しない場合はエラー
4. check_consistency(A, C, B) で 4 グループ評価 (§3)
5. domain-types.json に signals を書き戻し (既存)
6. サマリ出力に skipped_count を追加
```

- baseline がなければエラー: `baseline-capture` を先に実行するよう案内
- baseline は読み取り専用。このコマンドが baseline を変更することはない

#### ワークフロー

```
/track:design
  └→ sotp track baseline-capture <id>    # B を生成 (1 回)
  └→ sotp track domain-type-signals <id> # 評価 (B = C なので Red = 0)

/track:implement (実装中)
  └→ sotp track domain-type-signals <id> # 評価 (何度でも)
```

**制約**: track 進行中に main を track ブランチへマージすることは異常系であり、対応しない。main で追加された型がグループ 4 として Red になるが、これは想定外の操作の帰結であり回避策は提供しない。

### 5. verify spec-states への影響

`verify_from_spec_json()` は変更不要。upstream の signal 生成時にベースラインフィルタリングが適用されるため、`domain-types.json` に書き込まれる Red signals は「本当に検出すべき」ものだけになる。既存の Red ゲートロジック（Red > 0 → fail）はそのまま機能する。

### 7. レイヤー配置

各層に `tddd/` モジュールを作成し、TDDD 関連ファイルを集約する。新規ファイルの追加と既存ファイルの移動を本 track で行う。

| コンポーネント | レイヤー | 場所 | 備考 |
|---|---|---|---|
| `TypeBaseline` / `TypeBaselineEntry` / `TraitBaselineEntry` | domain | `libs/domain/src/tddd/baseline.rs` | 新規 |
| `check_consistency` (baseline 引数拡張) | domain | `libs/domain/src/tddd/consistency.rs` | `domain_types.rs` から移動 |
| `DomainTypeEntry` / `DomainTypesDocument` 等 | domain | `libs/domain/src/tddd/catalogue.rs` | `domain_types.rs` から移動 |
| Baseline codec (encode/decode) | infrastructure | `libs/infrastructure/src/tddd/baseline_codec.rs` | 新規 |
| TypeGraph → TypeBaseline 変換 | infrastructure | `libs/infrastructure/src/tddd/baseline_builder.rs` | 新規 |
| 型カタログ codec | infrastructure | `libs/infrastructure/src/tddd/catalogue_codec.rs` | `domain_types_codec.rs` から移動 |
| CLI `baseline-capture` コマンド | cli | `apps/cli/src/commands/track/tddd/baseline.rs` | 新規 |
| CLI `domain-type-signals` 拡張 | cli | `apps/cli/src/commands/track/tddd/signals.rs` | `domain_state_signals.rs` から移動 |

配置原則:
- 各層の `tddd/` モジュールに TDDD 関連を集約。既存ファイルも本 track で移動する
- 1 概念 1 ファイル: baseline / consistency / catalogue / codec / builder / signals をそれぞれ分離
- domain 層は純粋データ型 + 比較関数。I/O は infrastructure / CLI 層

## Rejected Alternatives

### A. git diff + regex で新規型名を抽出

`git diff main...HEAD -- libs/domain/src/` の出力を regex (`pub (struct|enum|trait) Name`) でスキャンし、新規型名を特定する。

却下理由:
- rustdoc JSON が既に型情報を完全に提供しているのに regex で再実装するのは車輪の再発明
- regex はプリプロセッサ、マクロ生成型、`pub(crate)` vs `pub` の区別を正しく扱えない
- 構造変更（variant 追加等）の検出には regex では不十分 — AST レベルの比較が必要
- git コンテキスト依存（main ブランチ不在、detached HEAD）で fragile

### B. main ブランチで 2 回目の rustdoc export

main と current の両方で `cargo +nightly rustdoc` を実行し、TypeGraph を比較する。

却下理由:
- rustdoc export は約 10 秒かかる — 2 倍の 20 秒は開発体験を著しく悪化させる
- main ブランチのチェックアウトが必要 — worktree 作成またはファイル書き換えが発生
- TypeGraph 構築には `domain-types.json` の typestate 名情報が必要 — main ブランチの domain-types.json と current のどちらを使うかが曖昧

### C. domain-types.json 内にベースラインを埋め込み

`domain-types.json` に `"baseline"` フィールドを追加する。

却下理由:
- ベースラインは 100+ エントリになり得る — declared types (10-20 エントリ) の可読性を圧倒
- ライフサイクルが異なる（baseline は不変、declared は evolving）のに同一ファイルに混在
- schema_version 変更が必要 — 既存 domain-types.json との migration コストが発生

### D. accept-list で既存型を手動除外

`domain-types.json` に `"accepted_existing": [...]` を追加。

却下理由:
- 131+ の型名を手動で列挙・保守する負担
- 自動化の本質を損なう（手動リスト管理は TDDD の自動検出と矛盾）
- 構造変更の検出ができない（名前の一致しか見ない）

### E. reverse Red を warning に降格（CI 非ブロック化）

`verify spec-states` で reverse Red を無視し、forward Red のみブロックする。

却下理由:
- ADR `2026-04-08-1800-reverse-signal-integration.md` §4 で kind_tag ベースの forward/reverse 区別を明示的に却下済み — 単一ゲートが設計意図
- 「テストなしのコード」を許容することは TDDD の根幹を否定する
- ベースライン方式なら reverse check を維持しつつノイズを排除できるため、降格は不要

## Consequences

### Good

- **ノイズ排除**: 既存 100+ 型が Red として出力されなくなり、本当に検出すべき新規型・構造変更のみが浮上する
- **構造変更の検出**: 型名の一致だけでなく、variant/field/method レベルの変更も検出できる
- **既存インフラの流用**: ベースラインキャプチャは `RustdocSchemaExporter` + `build_type_graph` の既存パイプラインを使用するため、新たなエクスポート機構の開発は不要
- **既存 verify gate 無変更**: `verify_from_spec_json` は変更不要。upstream でフィルタリングされた signals が書き込まれるため、gate ロジックはそのまま
- **ADR 2026-04-08-1800 との互換性**: 単一ゲート（Red > 0 → fail）の設計を維持。kind_tag 依存を導入しない
- **TDDD 原則の維持**: reverse check を無効化するのではなく、精度を向上させる

### Bad

- **track 進行中の main マージ非対応**: track ブランチに main をマージすると main で追加された型が Red になるが、異常系として対応しない
- **新ファイルの管理**: `domain-types-baseline.json` が track ディレクトリに追加される
- **rustdoc export が 2 回発生**: `baseline-capture` と `domain-type-signals` がそれぞれ独立に rustdoc export を実行する。`/track:design` で両方呼ばれる場合、約 20 秒 (10 秒 × 2) かかる
- **トレイト比較の解像度**: baseline のトレイト比較はメソッド名のみ。メソッドのシグネチャ変更（引数型の変更、async 化等）は検出できない。ADR `2026-04-11-0002-tddd-multilayer-extension.md` (TDDD-01) で `MethodDeclaration` 構造が導入された後、baseline のトレイト比較もシグネチャレベルに引き上げることを推奨する
- **同名型の衝突**: baseline は型名 (最終パスセグメント) をキーとする `HashMap` であり、異なるモジュールに同名の pub 型がある場合にエントリが衝突する。これは TypeGraph 自体の既存制約 (`code_profile_builder.rs` の collision warning) と同根であり、baseline 側だけでは解決できない。TypeGraph のキーイングを完全修飾名に変更する際に baseline も追従する

## Reassess When

- TypeGraph の構造が変わった場合（新しい型情報が追加された等）— baseline スキーマの更新が必要になる可能性
- track 進行中の main マージが頻発し対応が必要になった場合 — baseline リフレッシュ機構を検討
- per-track ではなく per-workspace のグローバル型カタログ（全 track の宣言を統合）が必要になった場合 — baseline の概念が根本から変わる
- rustdoc JSON が stabilize された場合 — nightly 依存が解消され、ベースラインキャプチャの CI 構成が簡素化できる
- ADR `2026-04-11-0002-tddd-multilayer-extension.md` (TDDD-01) が実装された場合 — baseline を per-layer 化し、トレイト比較を `MethodDeclaration` 構造に引き上げる

## Related

- **ADR `2026-04-08-1800-reverse-signal-integration.md`**: TDDD reverse signal の導入元。本 ADR はその改善であり、設計原則（単一ゲート、TDDD、自動追加禁止）を全て維持する
- **ADR `2026-04-08-0045-spec-code-consistency-check-design.md`**: `check_consistency` 関数の設計元
- **ADR `2026-04-11-0002-tddd-multilayer-extension.md`** (TDDD-01): 型カタログ多層化 + L1 シグネチャ検証の設計。本 ADR の実装後、TDDD-01 の一部として baseline を per-layer 化し、トレイト比較を `MethodDeclaration` ベースに拡張する
- **ADR `2026-04-11-0003-type-action-declarations.md`** (TDDD-03): 型アクション宣言 (add/modify/delete)。本 ADR のグループ 3 で「削除 → Red」となるケースに対し、`action: "delete"` による意図的削除の宣言手段を提供する
